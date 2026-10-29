use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use serde::{Deserialize, Serialize};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::delay_effect::DelayEffect;
use crate::audio_effects::effect_input_observer::{ControlMapping, Curve, EffectGateObserver, EffectInputObserver, GateMode, Release};
use crate::audio_effects::effect_slot::EffectSlot;
use crate::audio_effects::gain_effect::GainEffect;
use crate::audio_effects::low_pass_filter_effect::LowPassFilterEffect;
use crate::audio_effects::reverb_effect::ReverbEffect;
use crate::control_feedback::ControlFeedback;
use crate::control_input::{ControlInput, ControlInputObserver};
use crate::error::Error;
use crate::logger;
use crate::routing_director::RoutingDirector;

pub trait ExternalConnection {
  fn new(connection_str: &str) -> Result<Self, Error> where Self: Sized;
  fn start(
    self,
    routing_director: Arc<Mutex<RoutingDirector>>,
    control_inputs: Arc<Mutex<Vec<Box<dyn ControlInput>>>>
  );
}

static LOG_ENVIRONMENT: &str = "VcsgpConnection";

type MessageHandler = Arc<dyn Fn(&str) + Send + Sync>;

pub struct VcsgpConnection {
  listener: TcpListener,
}

impl VcsgpConnection {
  ///
  /// Accepts clients and handles each one on its own thread, so a client that hangs or
  /// drops without closing the socket cannot block newer connections.
  /// Every line received is one JSON message.
  ///
  fn listen(&self, handler: MessageHandler, feedback: Arc<ControlFeedback>) {
    logger::info(LOG_ENVIRONMENT, "listening to VCSGP connection");

    for stream in self.listener.incoming() {
      let stream = match stream {
        Ok(stream) => stream,
        Err(e) => {
          logger::error_str(LOG_ENVIRONMENT, &format!("could not accept connection: {}", e));
          continue;
        }
      };
      let peer = stream.peer_addr().map(|address| address.to_string()).unwrap_or_else(|_| "unknown".to_string());
      logger::info(LOG_ENVIRONMENT, &format!("client {} connected", peer));

      // The same connection carries live control states back to the panel
      match stream.try_clone() {
        Ok(writer) => feedback.add_client(peer.clone(), writer),
        Err(e) => logger::warn(LOG_ENVIRONMENT, &format!("no control feedback for {}: {}", peer, e)),
      }

      let handler = Arc::clone(&handler);
      thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
          match line {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => handler(&line),
            Err(e) => {
              logger::error_str(LOG_ENVIRONMENT, &format!("connection with {} failed: {}", peer, e));
              break;
            }
          }
        }
        logger::info(LOG_ENVIRONMENT, &format!("client {} disconnected", peer));
      });
    }
  }

  ///
  /// Parses a configuration message and applies it to the routing director.
  /// Effects are created before the routing director is locked, because creating them allocates
  /// (e.g. reverb delay lines) and the audio routing loop needs that lock.
  ///
  fn apply_configuration(
    data: &str,
    routing_director: &Mutex<RoutingDirector>,
    control_inputs: &Mutex<Vec<Box<dyn ControlInput>>>
  ) {
    let dto: Dto = match serde_json::from_str(data) {
      Ok(dto) => dto,
      Err(e) => {
        logger::error_str(LOG_ENVIRONMENT, &format!("invalid message: {}", e));
        return;
      }
    };

    let sample_rate = routing_director.lock().unwrap().sample_rate() as f64;
    let effect_chains: Vec<Vec<EffectSlot>> = dto.audio_buses
      .iter()
      .map(|bus_dto| Self::build_effect_chain(bus_dto, sample_rate, control_inputs))
      .collect();

    let mut rd_guard = routing_director.lock().unwrap();
    if let Some(main_volume) = dto.main_volume {
      rd_guard.set_main_volume(main_volume);
    }

    for (bus_dto, effects) in dto.audio_buses.iter().zip(effect_chains) {
      match rd_guard.audio_bus(&bus_dto.id) {
        None => logger::error_str(LOG_ENVIRONMENT, &format!("{} does not exist", bus_dto.id)),
        Some(bus) => {
          bus.set_effects(effects);
          if bus_dto.enabled { bus.enable() } else { bus.disable() }
        }
      }
    }
  }

  fn build_effect_chain(
    dto: &AudioBusDto,
    sample_rate: f64,
    control_inputs: &Mutex<Vec<Box<dyn ControlInput>>>
  ) -> Vec<EffectSlot> {
    dto.effects
      .iter()
      .filter_map(|effect_dto| {
        // An effect that can be switched on by a control input is always built, even when it starts off
        let gates: Vec<&EffectParameterDto> = effect_dto.parameters.iter()
          .filter(|p| p.key == Self::ENABLED_KEY && !p.input_control_id.is_empty())
          .collect();
        if !effect_dto.enabled && gates.is_empty() {
          return None;
        }

        let effect = match Self::build_effect(effect_dto, sample_rate) {
          Ok(effect) => effect,
          Err(e) => {
            logger::error(LOG_ENVIRONMENT, e);
            return None;
          }
        };

        // A momentary effect is only on while its control is touched
        let starts_active = effect_dto.enabled && !gates.iter().any(|gate| gate.mode == GateMode::Momentary);
        let slot = EffectSlot::new(effect, starts_active);
        Self::attach_control_inputs(&slot, &effect_dto.parameters, control_inputs);
        Some(slot)
      })
      .collect()
  }

  fn build_effect(dto: &EffectDto, sample_rate: f64) -> Result<Box<dyn AudioEffect>, Error> {
    let parameter = |key: &str| Self::get_parameter(key, &dto.parameters)
      .ok_or_else(|| Error::new(&format!("{} parameter not found for {:?}", key, dto.effect_type)));

    Ok(match dto.effect_type {
      EffectType::Gain => Box::new(GainEffect::new(dto.mix, parameter("gain")?, sample_rate)),
      EffectType::LowPassFilter => Box::new(LowPassFilterEffect::new(
        dto.mix,
        parameter("frequency")?,
        parameter("q_factor")?,
        sample_rate,
      )),
      EffectType::Reverb => Box::new(ReverbEffect::new(
        dto.mix,
        parameter("room_size")?,
        parameter("decay")?,
        parameter("dampening")?,
        sample_rate,
      )),
      EffectType::Delay => Box::new(DelayEffect::new(dto.mix, parameter("delay")?, sample_rate)),
    })
  }

  ///
  /// Registers the effect's parameters with the control inputs (e.g. a rotary encoder) they are mapped to.
  /// Parameters without an `input_control_id` are not mapped to hardware.
  ///
  fn attach_control_inputs(
    slot: &EffectSlot,
    parameters: &[EffectParameterDto],
    control_inputs: &Mutex<Vec<Box<dyn ControlInput>>>
  ) {
    let ci_guard = control_inputs.lock().unwrap();

    for parameter_dto in parameters.iter().filter(|p| !p.input_control_id.is_empty()) {
      let Some(control_input) = ci_guard.iter().find(|ci| ci.id() == parameter_dto.input_control_id) else {
        logger::warn(LOG_ENVIRONMENT, &format!("control input with id [{}] not found", parameter_dto.input_control_id));
        continue;
      };

      match Self::create_observer(slot, parameter_dto) {
        Ok(observer) => {
          control_input.observable().register(observer);
          logger::info(LOG_ENVIRONMENT, &format!(
            "{} controls {} of {}", parameter_dto.input_control_id, parameter_dto.key, slot.effect().get_type()
          ));
        }
        Err(e) => logger::error_str(
          LOG_ENVIRONMENT,
          &format!("{} cannot be controlled by {}: {}", parameter_dto.key, parameter_dto.input_control_id, e.message)
        ),
      }
    }
  }

  /// The `enabled` key switches the whole effect on and off; any other key controls that parameter.
  fn create_observer(slot: &EffectSlot, parameter_dto: &EffectParameterDto) -> Result<Arc<dyn ControlInputObserver>, Error> {
    if parameter_dto.key == Self::ENABLED_KEY {
      return Ok(Arc::new(EffectGateObserver::new(&slot.active(), parameter_dto.mode)));
    }

    let storage = slot.effect().parameter(&parameter_dto.key)?;
    Ok(Arc::new(EffectInputObserver::new(&storage, parameter_dto.mapping())))
  }

  fn get_parameter(key: &str, parameters: &[EffectParameterDto]) -> Option<u16> {
    parameters.iter()
      .find(|p| p.key == key)
      .map(|p| p.value)
  }
}

impl VcsgpConnection {
  /// Parameter key that binds a control input to switching an effect on and off.
  const ENABLED_KEY: &str = "enabled";
}

impl ExternalConnection for VcsgpConnection {
  ///
  /// Opens a new VCSGP (Very Cool and Shitty Guitar Protocol) listener using a TCP/IP socket.
  ///
  /// `connections_str` takes a connection string with the following info:
  /// `[ip_address]:[port_number]`.
  ///
  fn new(connection_str: &str) -> Result<Self, Error>
  where
    Self: Sized
  {
    let listener = TcpListener::bind(connection_str)
      .map_err(|e| Error::new(&format!("could not create TCP socket on {}: {}", connection_str, e)))?;

    logger::info(LOG_ENVIRONMENT, "VCSGP listener created");

    Ok(Self { listener })
  }

  fn start(
    self,
    routing_director: Arc<Mutex<RoutingDirector>>,
    control_inputs: Arc<Mutex<Vec<Box<dyn ControlInput>>>>
  ) {
    logger::info(LOG_ENVIRONMENT, "starting to listen to VCSGP");

    let feedback = ControlFeedback::start(control_inputs.clone());
    thread::spawn(move || {
      self.listen(Arc::new(move |data| {
        Self::apply_configuration(data, &routing_director, &control_inputs);
      }), feedback);
    });
  }
}

///
/// A complete configuration sent by the panel, one JSON object per line.
/// See `api_contract.json` for an example.
///
#[derive(Serialize, Deserialize)]
struct Dto {
  /// Volume applied after every bus' effect chain, `0` is silent and `65535` is unity gain.
  #[serde(default)]
  main_volume: Option<u16>,
  audio_buses: Vec<AudioBusDto>,
}

#[derive(Serialize, Deserialize)]
struct AudioBusDto {
  id: String,
  enabled: bool,
  effects: Vec<EffectDto>,
}

#[derive(Serialize, Deserialize)]
struct EffectDto {
  effect_type: EffectType,
  mix: u16,
  /// Disabled effects are left out of the chain.
  #[serde(default = "default_enabled")]
  enabled: bool,
  parameters: Vec<EffectParameterDto>,
}

fn default_enabled() -> bool {
  true
}

///
/// A parameter of an effect. With an `input_control_id` it is also a binding: that control input
/// (e.g. `"laser_0"`) changes the parameter live, shaped by the optional binding fields.
/// The key `"enabled"` binds the control to switching the effect on and off instead.
///
#[derive(Serialize, Deserialize)]
struct EffectParameterDto {
  key: String,
  /// Value of the parameter; with `release: "return"` the parameter goes back to it.
  value: u16,
  /// Id of the control input mapped to this parameter, e.g. `"rotary_0"`. Empty or absent means unmapped.
  #[serde(default)]
  input_control_id: String,
  /// Parameter value at the start of the control's range (default `0`).
  #[serde(default)]
  range_min: Option<u16>,
  /// Parameter value at the end of the control's range (default `65535`); may be below `range_min` to invert.
  #[serde(default)]
  range_max: Option<u16>,
  #[serde(default)]
  curve: Curve,
  /// What happens when a laser string is released: `"hold"` (default) or `"return"`.
  #[serde(default)]
  release: Release,
  /// For the `"enabled"` key: `"momentary"` (default, on while touched) or `"toggle"`.
  #[serde(default)]
  mode: GateMode,
}

impl EffectParameterDto {
  fn mapping(&self) -> ControlMapping {
    ControlMapping {
      range_min: self.range_min.unwrap_or(0),
      range_max: self.range_max.unwrap_or(u16::MAX),
      curve: self.curve,
      release: self.release,
      base: self.value,
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum EffectType {
  Gain,
  LowPassFilter,
  Reverb,
  Delay,
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE_RATE: f64 = 48000.0;

  #[test]
  fn api_contract_builds_every_effect() {
    let dto: Dto = serde_json::from_str(include_str!("../api_contract.json")).unwrap();
    let effects: Vec<&EffectDto> = dto.audio_buses.iter().flat_map(|bus| &bus.effects).collect();
    assert_eq!(effects.len(), 4);

    for effect_dto in effects {
      let mut effect = VcsgpConnection::build_effect(effect_dto, SAMPLE_RATE)
        .unwrap_or_else(|e| panic!("{:?}: {}", effect_dto.effect_type, e.message));
      assert!(effect.process_chunk(0.5).is_finite());

      // Every parameter the panel maps to hardware must be controllable live
      let slot = EffectSlot::new(effect, true);
      for parameter in effect_dto.parameters.iter().filter(|p| !p.input_control_id.is_empty()) {
        assert!(
          VcsgpConnection::create_observer(&slot, parameter).is_ok(),
          "{:?} cannot be controlled through {}", effect_dto.effect_type, parameter.key
        );
      }
    }
  }

  #[test]
  fn missing_parameter_is_rejected() {
    let dto: EffectDto = serde_json::from_str(
      r#"{"effect_type": "low_pass_filter", "mix": 65535, "parameters": [{"key": "frequency", "value": 3273}]}"#
    ).unwrap();
    assert!(VcsgpConnection::build_effect(&dto, SAMPLE_RATE).is_err());
  }

  #[test]
  fn optional_fields_have_defaults() {
    let dto: Dto = serde_json::from_str(
      r#"{"audio_buses": [{"id": "bus_0", "enabled": true, "effects": [
        {"effect_type": "gain", "mix": 65535, "parameters": [{"key": "gain", "value": 16384}]}
      ]}]}"#
    ).unwrap();
    assert_eq!(dto.main_volume, None);
    let effect = &dto.audio_buses[0].effects[0];
    assert!(effect.enabled);
    assert!(effect.parameters[0].input_control_id.is_empty());
    assert_eq!(effect.parameters[0].mapping(), ControlMapping::full_range(16384));
    assert_eq!(effect.parameters[0].mode, GateMode::Momentary);
  }

  #[test]
  fn binding_options_are_parsed() {
    let parameter: EffectParameterDto = serde_json::from_str(
      r#"{"key": "frequency", "value": 3273, "input_control_id": "laser_0",
          "range_min": 40000, "range_max": 1000, "curve": "exponential", "release": "return"}"#
    ).unwrap();
    assert_eq!(parameter.mapping(), ControlMapping {
      range_min: 40000, range_max: 1000, curve: Curve::Exponential, release: Release::Return, base: 3273,
    });

    let gate: EffectParameterDto = serde_json::from_str(
      r#"{"key": "enabled", "value": 65535, "input_control_id": "laser_1", "mode": "toggle"}"#
    ).unwrap();
    assert_eq!(gate.mode, GateMode::Toggle);
  }

  fn chain_for(effects_json: &str) -> Vec<EffectSlot> {
    let bus: AudioBusDto = serde_json::from_str(
      &format!(r#"{{"id": "bus_0", "enabled": true, "effects": [{}]}}"#, effects_json)
    ).unwrap();
    VcsgpConnection::build_effect_chain(&bus, SAMPLE_RATE, &Mutex::new(Vec::new()))
  }

  #[test]
  fn momentary_effect_starts_off_and_disabled_effect_is_left_out() {
    let gain = |enabled: bool, extra: &str| format!(
      r#"{{"effect_type": "gain", "enabled": {}, "mix": 65535, "parameters": [{{"key": "gain", "value": 16384}}{}]}}"#,
      enabled, extra
    );

    assert!(chain_for(&gain(false, "")).is_empty(), "disabled without a control is left out");

    let momentary = chain_for(&gain(true, r#", {"key": "enabled", "value": 65535, "input_control_id": "laser_0"}"#));
    assert!(!momentary[0].active().load(std::sync::atomic::Ordering::Relaxed), "momentary starts off");

    let toggle = chain_for(&gain(false, r#", {"key": "enabled", "value": 0, "input_control_id": "laser_0", "mode": "toggle"}"#));
    assert_eq!(toggle.len(), 1, "a toggled effect is built even when it starts off");
    assert!(!toggle[0].active().load(std::sync::atomic::Ordering::Relaxed));
  }
}
