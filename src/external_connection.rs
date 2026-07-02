use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use serde::{Deserialize, Serialize};
use crate::audio_bus::AudioBus;
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

pub struct VcsgpConnection {
  listener: TcpListener,
}

impl VcsgpConnection {
  fn listen(&self, callback: Box<dyn Fn(&str)>) {
    logger::info(LOG_ENVIRONMENT, "listening to VCSGP connection");

    for stream in self.listener.incoming().map(|s| s.unwrap()) {
      let reader = BufReader::new(stream);

      for line in reader.lines().map(|l| l.unwrap()) {
        callback(&line);
      }
    }
  }

  fn update_bus(
    audio_bus: &mut AudioBus,
    dto: &AudioBusDto,
    control_inputs: &Arc<Mutex<Vec<Box<dyn ControlInput>>>>
  ) {
    if dto.enabled { audio_bus.enable() }
    else { audio_bus.disable() }

    audio_bus.clear_effects();
    dto.effects.iter().for_each(|effect_dto| {
      // Create new effect instance
      let mut effect: Box<dyn AudioEffect> = match effect_dto.effect_type {
        EffectType::Gain => {
          let gain = match Self::get_parameter("gain", effect_dto.parameters) {
            Some(gain) => gain,
            None => { logger::error_str(LOG_ENVIRONMENT, "gain parameter not found"); return },
          };
          Box::new(GainEffect::new(effect_dto.mix, gain))
        }
        EffectType::LowPassFilter => {
          let frequency = match Self::get_parameter("frequency", effect_dto.parameters) {
            Some(frequency) => frequency,
            None => { logger::error_str(LOG_ENVIRONMENT, "frequency parameter not found"); return },
          };
          let q_factor = match Self::get_parameter("q_factor", effect_dto.parameters) {
            Some(q_factor) => q_factor,
            None => { logger::error_str(LOG_ENVIRONMENT, "q_factor parameter not found"); return },
          };
          Box::new(LowPassFilterEffect::new(effect_dto.mix, frequency, q_factor))
        },
        EffectType::Reverb => {
          let room_size = match Self::get_parameter("room_size", effect_dto.parameters) {
            Some(room_size) => room_size,
            None => { logger::error_str(LOG_ENVIRONMENT, "room_size parameter not found"); return },
          };
          let reverb_decay = match Self::get_parameter("decay", effect_dto.parameters) {
            Some(reverb_decay) => reverb_decay,
            None => { logger::error_str(LOG_ENVIRONMENT, "decay parameter not found"); return },
          };
          let dampening = match Self::get_parameter("dampening", effect_dto.parameters) {
            Some(dampening) => dampening,
            None => { logger::error_str(LOG_ENVIRONMENT, "dampening parameter not found"); return },
          };
          Box::new(ReverbEffect::new(effect_dto.mix, room_size, reverb_decay, dampening))
        },
        EffectType::Delay => {
          let delay = match Self::get_parameter("delay", effect_dto.parameters) {
            Some(delay) => delay,
            None => { logger::error_str(LOG_ENVIRONMENT, "delay parameter not found"); return },
          };
          Box::new(DelayEffect::new(effect_dto.mix, delay))
        },
      };

      // Set effect parameters and attach control inputs
      effect_dto.parameters.iter().for_each(|parameter_dto| {
        let observer = match effect.get_control_observer(parameter_dto.key.as_str()) {
          Ok(observer) => observer,
          Err(e) => {
            logger::error(LOG_ENVIRONMENT, e);
            return;
          },
        };
        let ci_guard = control_inputs.lock().unwrap();
        let control_input = match ci_guard
          .iter().find(|ci| ci.id() == parameter_dto.input_control_id) {
          Some(ci) => ci,
          None => {
            logger::error_str(LOG_ENVIRONMENT, &format!("control input with id [{}] not found", parameter_dto.input_control_id));
            return;
          },
        };
        let observable = control_input.observable();
        observable.register(observer);
      });

      audio_bus.add_effect(effect);
    });
  }

  fn get_parameter(key: &str, parameters: Vec<EffectParameterDto>) -> Option<u16> {
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
    let listener = match TcpListener::bind(connection_str) {
      Ok(listener) => listener,
      Err(_) => return Err(Error::new("could not create TCP socket"))
    };

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
      self.listen(Box::new(move |data| {
        let data = data.to_owned();
        let thread_routing_director = routing_director.clone();
        let control_inputs = Arc::clone(&control_inputs);

        // Convert incoming data to JSON
        let dto: Dto = match serde_json::from_str(&*data) {
          Ok(dto) => dto,
          Err(e) => {
            logger::error_str(LOG_ENVIRONMENT, &e.to_string());
            return;
          }
        };

        // Add audio effects
        let mut audio_bus_dto = dto.audio_buses;
        let mut rd_guard = thread_routing_director.lock().unwrap();
        audio_bus_dto.iter().for_each(|dto| {
          match rd_guard.audio_buses().iter_mut().find(|bus| bus.id() == dto.id) {
            None => logger::error_str(LOG_ENVIRONMENT, &format!("{} does not exist", dto.id)),
            Some(bus) => Self::update_bus(bus, dto, &control_inputs),
          }
        });
      }));
    });
  }
}

#[derive(Serialize, Deserialize)]
struct Dto {
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
  parameters: Vec<EffectParameterDto>,
}

#[derive(Serialize, Deserialize)]
struct EffectParameterDto {
  key: String,
  value: u16,
  input_control_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectType {
  Gain,
  LowPassFilter,
  Reverb,
  Delay,
}
