use std::io::Read;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use serde_json::{Value};
use serde::{Deserialize, Serialize};
use crate::audio_bus::AudioBus;
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::gain_effect::GainEffect;
use crate::audio_effects::low_pass_filter_effect::LowPassFilterEffect;
use crate::audio_effects::reverb_effect::ReverbEffect;
use crate::control_input::{ControlInput, RotaryInput};
use crate::error::Error;
use crate::portal::external_connection::ExternalConnection;
use crate::routing_director::RoutingDirector;

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

  fn update_bus(audio_bus: &mut AudioBus, dto: &AudioBusDto, control_inputs: &mut Vec<dyn ControlInput>) {
    if dto.enabled { audio_bus.enable() }
    else { audio_bus.disable() }

    audio_bus.clear_effects();
    dto.effects.iter().for_each(|effect_dto| {
      match effect_dto.effect_type {
        EffectType::Gain => {
          audio_bus.add_effect(Box::new(GainEffect::new()));
          let effects_len = audio_bus.effects().lock().unwrap().len();
          audio_bus.for_effect(effects_len - 1, |effect| {
            _ = Self::set_effect_value("gain", &effect_dto.parameters, effect);
          }).expect("Failed to change gain values");
        }
        EffectType::LowPassFilter => {
          audio_bus.add_effect(Box::new(LowPassFilterEffect::new()));
          let effects_len = audio_bus.effects().lock().unwrap().len();
          audio_bus.for_effect(effects_len - 1, |effect| {
            _ = Self::set_effect_value("frequency", &effect_dto.parameters, effect);
            _ = Self::set_effect_value("q_factor", &effect_dto.parameters, effect);
          }).expect("Failed to change low pass values");
        }
        EffectType::Reverb => {
          audio_bus.add_effect(Box::new(ReverbEffect::new()));
          let effects_len = audio_bus.effects().lock().unwrap().len();
          audio_bus.for_effect(effects_len - 1, |effect| {
            _ = Self::set_effect_value("room_size", &effect_dto.parameters, effect);
            _ = Self::set_effect_value("reverb_decay", &effect_dto.parameters, effect);
            _ = Self::set_effect_value("diffusion", &effect_dto.parameters, effect);
          }).expect("Failed to change reverb values");
        }
      }
    })
  }

  fn set_effect_value(key: &str, parameters: &Vec<Value>, effect: &mut dyn AudioEffect) -> Result<(), Error> {
    let value_u64 = parameters[key].as_u64();
    let value_u16 = match value_u64 {
      Some(value) => value as u16,
      None() => return Err(Error::new(format!("{} is not a u16", key).as_str())),
    };
    effect.set_value(key, value_u16)
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
    input_controls: Arc<Mutex<Vec<dyn ControlInput>>>
  ) {
    self.listen(Box::new(move |data| {
      let dto: Dto = serde_json::from_str(data).unwrap();

      // Add control inputs
      dto.control_inputs.iter().for_each(|control_input_dto| {
        match control_input_dto.control_type {
          ControlType::Rotary => {
            input_controls.lock().unwrap().push(RotaryInput::new());
          }
        }
      });

      // Add audio effects
      let audio_bus_dto = dto.audio_buses;
      let mut audio_buses = routing_director
          .lock()
          .unwrap()
          .audio_buses()
          .iter_mut().for_each(|bus| {
            audio_bus_dto.iter().for_each(|dto| {
              if bus.id() == dto.id {
                Self::update_bus(bus, dto);
              }
            });
          });
    }));
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
  effect_type: EffectType,
  parameters: Vec<Value>,
}

#[derive(Serialize, Deserialize)]
enum EffectType {
  Gain,
  LowPassFilter,
  Reverb,
}
