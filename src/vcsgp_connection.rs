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
use crate::error::Error;
use crate::external_connection::ExternalConnection;
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

  fn update_bus(audio_bus: &mut AudioBus, dto: &AudioBusDto) {
    if dto.enabled { audio_bus.enable() }
    else { audio_bus.disable() }

    audio_bus.clear_effects();
    dto.effects.iter().for_each(|effect_dto| {
      match effect_dto.effect_type {
        EffectType::Gain => {
          audio_bus.add_effect(Box::new(GainEffect::new()));
          let effects_len = audio_bus.effects().lock().unwrap().len();
          audio_bus.for_effect(effects_len - 1, |effect| {
            let gain_value = effect_dto
                .parameters["gain"]
                .as_u64()
                .unwrap() as u16;
            _ = effect.set_value("gain", gain_value);
          }).expect("Failed to change gain values");
        }
        EffectType::LowPassFilter => {
          audio_bus.add_effect(Box::new(LowPassFilterEffect::new()));
          let effects_len = audio_bus.effects().lock().unwrap().len();
          audio_bus.for_effect(effects_len - 1, |effect| {
            let frequency_value = effect_dto
                .parameters["frequency"]
                .as_u64()
                .unwrap() as u16;
            _ = effect.set_value("frequency", frequency_value);
            let q_factor_value = effect_dto
                .parameters["q_factor"]
                .as_u64()
                .unwrap() as u16;
            _ = effect.set_value("q_factor", q_factor_value);
          }).expect("Failed to change low pass values");
        }
        EffectType::Reverb => {
          audio_bus.add_effect(Box::new(ReverbEffect::new()));
          let effects_len = audio_bus.effects().lock().unwrap().len();
          audio_bus.for_effect(effects_len - 1, |effect| {
            let room_size_value = effect_dto
                .parameters["room_size"]
                .as_u64()
                .unwrap() as u16;
            _ = effect.set_value("room_size", room_size_value);
            let reverb_decay_value = effect_dto
                .parameters["reverb_decay"]
                .as_u64()
                .unwrap() as u16;
            _ = effect.set_value("reverb_decay", reverb_decay_value);
            let diffusion_value = effect_dto
                .parameters["diffusion"]
                .as_u64()
                .unwrap() as u16;
            _ = effect.set_value("diffusion", diffusion_value);
          }).expect("Failed to change reverb values");
        }
      }
    })
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

  fn start(&mut self, routing_director: Arc<Mutex<RoutingDirector>>) {
    self.listen(Box::new(move |data| {
      let audio_bus_dto: Vec<AudioBusDto> = serde_json::from_str(data).unwrap();

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
struct AudioBusDto {
  id: String,
  enabled: bool,
  effects: Vec<EffectDto>,
}

#[derive(Serialize, Deserialize)]
struct EffectDto {
  effect_type: EffectType,
  parameters: Value,
}

#[derive(Serialize, Deserialize)]
enum EffectType {
  Gain,
  LowPassFilter,
  Reverb,
}
