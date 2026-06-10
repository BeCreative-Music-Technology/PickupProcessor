use std::io::Read;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use serde::{Deserialize, Serialize};
use crate::audio_bus::AudioBus;
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::gain_effect::GainEffect;
use crate::audio_effects::low_pass_filter_effect::LowPassFilterEffect;
use crate::audio_effects::reverb_effect::ReverbEffect;
use crate::control_input::{ControlInput, RotaryInput};
use crate::error::Error;
use crate::logger;
use crate::portal::external_connection::ExternalConnection;
use crate::routing_director::RoutingDirector;

static LOG_ENVIRONMENT: &str = "VcsgpConnection";

pub struct VcsgpConnection {
  listener: TcpListener,
}

impl VcsgpConnection {
  fn listen(&self, callback: Box<dyn Fn(&str)>) {
    for stream in self.listener.incoming() {
      let Ok(mut stream) = stream else {
        continue;
      };

      let mut buffer = Vec::new();
      if stream.read_to_end(&mut buffer).is_ok() && !buffer.is_empty() {
        let message = String::from_utf8_lossy(&buffer);
        callback(&message);
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
        EffectType::Gain => Box::new(GainEffect::new()),
        EffectType::LowPassFilter => Box::new(LowPassFilterEffect::new()),
        EffectType::Reverb => Box::new(ReverbEffect::new()),
      };

      // Set effect parameters and attach control inputs
      effect_dto.parameters.iter().for_each(|parameter_dto| {
        _ = effect.set_value(parameter_dto.key.as_str(), parameter_dto.value);

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
      Err(_) => return Err(Error::new("Could not create TCP socket"))
    };

    Ok(Self { listener })
  }

  fn start(
    &mut self,
    routing_director: Arc<Mutex<RoutingDirector>>,
    control_inputs: Arc<Mutex<Vec<Box<dyn ControlInput>>>>
  ) {
    thread::spawn(move || {
      self.listen(Box::new(move |data| {
        // Convert incoming data to JSON
        let dto: Dto = match serde_json::from_str(data) {
          Ok(dto) => dto,
          Err(e) => {
            logger::error_str(LOG_ENVIRONMENT, &e.to_string());
            return;
          }
        };

        // Add control inputs
        dto.control_inputs.iter().for_each(|control_input_dto| {
          match control_input_dto.control_type {
            ControlType::Rotary => {
              control_inputs.lock().unwrap().push(Box::new(RotaryInput::new()));
            }
          }
        });

        // Add audio effects
        let audio_bus_dto = dto.audio_buses;
        routing_director.lock().unwrap()
          .audio_buses()
          .iter_mut().for_each(|bus| {
          audio_bus_dto.iter().for_each(|dto| {
            if bus.id() == dto.id {
              Self::update_bus(bus, dto, &control_inputs);
            }
          });
        });
      }));
    });
  }
}

#[derive(Serialize, Deserialize)]
struct Dto {
  control_inputs: Vec<ControlInputDto>,
  audio_buses: Vec<AudioBusDto>,
}

#[derive(Serialize, Deserialize)]
struct ControlInputDto {
  id: String,
  control_type: ControlType,
}

#[derive(Serialize, Deserialize)]
enum ControlType {
  Rotary
}

#[derive(Serialize, Deserialize)]
struct AudioBusDto {
  id: String,
  enabled: bool,
  effects: Vec<EffectDto>,
}

#[derive(Serialize, Deserialize)]
struct EffectDto {
  id: String,
  effect_type: EffectType,
  parameters: Vec<EffectParameterDto>,
}

#[derive(Serialize, Deserialize)]
struct EffectParameterDto {
  key: String,
  value: u16,
  input_control_id: String,
}

#[derive(Serialize, Deserialize)]
enum EffectType {
  Gain,
  LowPassFilter,
  Reverb,
}
