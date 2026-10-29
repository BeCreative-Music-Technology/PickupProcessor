use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use fundsp::Frame;
use fundsp::numeric_array::generic_array::GenericArray;
use fundsp::prelude32::{reverb_stereo, U2};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_helper::Smoothed;
use crate::error::Error;
use crate::logger;

pub struct ReverbEffect
{
  mix: Arc<AtomicU16>, // 0..1 (percentage)
  reverb: Box<dyn FnMut(f32) -> f32 + Send>,
  smoothed_mix: Smoothed,
}

static LOG_ENVIRONMENT: &str = "ReverbEffect";

impl ReverbEffect {
  ///
  /// Creates a new reverb.
  /// Typical values: `room_size` u16::MAX / 2 (20 meters), `reverb_decay` 6524 (2 seconds),
  /// `dampening` u16::MAX / 2 (50%).
  ///
  pub fn new(mix: u16, room_size: u16, reverb_decay: u16, dampening: u16, sample_rate: f64) -> Self
  where
    Self: Sized
  {
    logger::info(LOG_ENVIRONMENT, "effect created");

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      reverb: Self::reverb_factory(room_size, reverb_decay, dampening, sample_rate),
      smoothed_mix: Smoothed::new(Self::parse_mix(mix), sample_rate),
    }
  }

  fn parse_mix(mix: u16) -> f32 {
    effect_helper::map(mix, u16::MIN, u16::MAX, 0.0, 1.0)
  }

  fn reverb_factory(room_size: u16, reverb_decay: u16, dampening: u16, sample_rate: f64) -> Box<dyn FnMut(f32) -> f32 + Send> {
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
    reverb.set_sample_rate(sample_rate);

    Box::new(move |sample: f32| {
      reverb.tick(&Frame::new(GenericArray::<f32, U2>::from_array([sample, sample])))[0]
    })
  }
}

impl AudioEffect for ReverbEffect {
  fn process_chunk(&mut self, sample: f32) -> f32 {
    let mix = self.smoothed_mix.next(Self::parse_mix(self.mix.load(Ordering::Relaxed)));

    effect_helper::mix(sample, (self.reverb)(sample), mix)
  }

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    self.parameter(key)?.store(value, Ordering::Relaxed);
    logger::info(LOG_ENVIRONMENT, &format!("set parameter {} to {}", key, value));
    Ok(())
  }

  fn parameter(&self, key: &str) -> Result<Arc<AtomicU16>, Error> {
    match key {
      "mix" => Ok(self.mix.clone()),
      _ => Err(Error::new("Parameter not found")),
    }
  }

  fn get_type(&self) -> &str {
    "reverb"
  }
}
