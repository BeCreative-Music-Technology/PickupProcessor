use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use fundsp::Frame;
use fundsp::numeric_array::generic_array::GenericArray;
use fundsp::prelude32::{reverb_stereo, U2};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_input_observer::EffectInputObserver;
use crate::control_input::ControlInputObserver;
use crate::error::Error;
use crate::logger;

pub struct ReverbEffect
{
  mix: Arc<AtomicU16>, // 0..1 (percentage)
  reverb: Box<dyn FnMut(f32) -> f32 + Send>,
}

static LOG_ENVIRONMENT: &str = "ReverbEffect";

impl ReverbEffect {
  pub fn new(mix: u16, room_size: u16, reverb_decay: u16, dampening: u16) -> Self
  where
    Self: Sized
  {
    let room_size = room_size; // u16::MAX / 2; // 20 meters
    let reverb_decay = reverb_decay; // 6524; // 2 seconds
    let dampening = dampening; // u16::MAX / 2; // 50%

    logger::info(LOG_ENVIRONMENT, "effect created");

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      reverb: Self::reverb_factory(room_size, reverb_decay, dampening),
    }
  }

  fn reverb_factory(room_size: u16, reverb_decay: u16, dampening: u16) -> Box<dyn FnMut(f32) -> f32 + Send> {
    let room_size = effect_helper::map(
      room_size,
      u16::MIN,
      u16::MAX,
      10.0,
      30.0
    );
    let reverb_decay = effect_helper::map(
      reverb_decay,
      u16::MIN,
      u16::MAX,
      0.01,
      20.0
    );
    let dampening = effect_helper::map(
      dampening,
      u16::MIN,
      u16::MAX,
      0.0,
      1.0
    );

    let mut reverb = reverb_stereo(room_size, reverb_decay, dampening);

    Box::new(move |sample: f32| {
      reverb.tick(&Frame::new(GenericArray::<f32, U2>::from_array([sample, sample])))[0]
    })
  }
}

impl AudioEffect for ReverbEffect {
  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]> {
    let mix = effect_helper::map(
      self.mix.load(Ordering::Relaxed),
      u16::MIN,
      u16::MAX,
      0.0,
      1.0
    );

    chunk
        .into_iter()
        .map(|sample| {
          effect_helper::mix(sample, (self.reverb)(sample), mix)
        })
        .collect::<Vec<f32>>()
        .into_boxed_slice()
  }

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    match key {
      "mix" => self.mix.store(value, Ordering::Relaxed),
      _ => return Err(Error::new("Unknown parameter")),
    };
    logger::info(LOG_ENVIRONMENT, &format!("set parameter {} to {}", key, value));
    Ok(())
  }

  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error> {
    let value = match key {
      "mix" => &self.mix,
      _ => return Err(Error::new("Parameter not found")),
    };
    Ok(Arc::new(EffectInputObserver {
      value_storage: Arc::clone(value),
    }))
  }

  fn get_type(&self) -> &str {
    "reverb"
  }
}
