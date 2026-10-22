use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use serde::{Deserialize, Serialize};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::delay_effect::DelayEffect;
use crate::audio_effects::gain_effect::GainEffect;
use crate::audio_effects::low_pass_filter_effect::LowPassFilterEffect;
use crate::audio_effects::reverb_effect::ReverbEffect;
use crate::control_input::ControlInput;
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
  fn listen(&self, handler: MessageHandler) {
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
    let effect_chains: Vec<Vec<Box<dyn AudioEffect>>> = dto.audio_buses
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
  ) -> Vec<Box<dyn AudioEffect>> {
    dto.effects
      .iter()
      .filter(|effect_dto| effect_dto.enabled)
      .filter_map(|effect_dto| match Self::build_effect(effect_dto, sample_rate) {
        Ok(mut effect) => {
          Self::attach_control_inputs(effect.as_mut(), &effect_dto.parameters, control_inputs);
          Some(effect)
        }
        Err(e) => {
          logger::error(LOG_ENVIRONMENT, e);
          None
        }
      })
      .collect()
  }

  fn build_effect(dto: &EffectDto, sample_rate: f64) -> Result<Box<dyn AudioEffect>, Error> {
    let parameter = |key: &str| Self::get_parameter(key, &dto.parameters)
      .ok_or_else(|| Error::new(&format!("{} parameter not found for {:?}", key, dto.effect_type)));

    Ok(match dto.effect_type {
      EffectType::Gain => Box::new(GainEffect::new(dto.mix, parameter("gain")?)),
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
    effect: &mut dyn AudioEffect,
    parameters: &[EffectParameterDto],
    control_inputs: &Mutex<Vec<Box<dyn ControlInput>>>
  ) {
    let ci_guard = control_inputs.lock().unwrap();

    for parameter_dto in parameters.iter().filter(|p| !p.input_control_id.is_empty()) {
      let Some(control_input) = ci_guard.iter().find(|ci| ci.id() == parameter_dto.input_control_id) else {
        logger::warn(LOG_ENVIRONMENT, &format!("control input with id [{}] not found", parameter_dto.input_control_id));
        continue;
      };

      match effect.get_control_observer(&parameter_dto.key) {
        Ok(observer) => control_input.observable().register(observer),
        Err(e) => logger::error_str(
          LOG_ENVIRONMENT,
          &format!("{} cannot be controlled by {}: {}", parameter_dto.key, parameter_dto.input_control_id, e.message)
        ),
      }
    }
  }

  fn get_parameter(key: &str, parameters: &[EffectParameterDto]) -> Option<u16> {
    parameters.iter()
      .find(|p| p.key == key)
      .map(|p| p.value)
  }
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

    thread::spawn(move || {
      self.listen(Arc::new(move |data| {
        Self::apply_configuration(data, &routing_director, &control_inputs);
      }));
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

#[derive(Serialize, Deserialize)]
struct EffectParameterDto {
  key: String,
  value: u16,
  /// Id of the control input mapped to this parameter, e.g. `"rotary_0"`. Empty or absent means unmapped.
  #[serde(default)]
  input_control_id: String,
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
      for parameter in effect_dto.parameters.iter().filter(|p| !p.input_control_id.is_empty()) {
        assert!(
          effect.get_control_observer(&parameter.key).is_ok(),
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
  }
}
