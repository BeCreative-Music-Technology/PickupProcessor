use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use fundsp::prelude32::{delay, An, Delay};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_helper::Smoothed;
use crate::error::Error;
use crate::logger;

static LOG_ENVIRONMENT: &str = "DelayEffect";

pub struct DelayEffect {
  mix: Arc<AtomicU16>,
  delay: An<Delay>,
  smoothed_mix: Smoothed,
}

impl DelayEffect {
  pub fn new(mix: u16, delay_value: u16, sample_rate: f64) -> Self
  where
    Self: Sized
  {
    logger::info(LOG_ENVIRONMENT, "effect created");

    let delay_value = effect_helper::map(
      delay_value, // AtomicU16::new(16137) // 0.5 seconds
      u16::MIN,
      u16::MAX,
      0.01,
      2.0
    );

    let mut delay = delay(delay_value);
    delay.set_sample_rate(sample_rate);

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      delay,
      smoothed_mix: Smoothed::new(Self::parse_mix(mix), sample_rate),
    }
  }

  fn parse_mix(mix: u16) -> f32 {
    effect_helper::map(mix, u16::MIN, u16::MAX, 0.0, 1.0)
  }
}

impl AudioEffect for DelayEffect {
  fn process_chunk(&mut self, sample: f32) -> f32 {
    let mix = self.smoothed_mix.next(Self::parse_mix(self.mix.load(Ordering::Relaxed)));

    let processed = self.delay.tick(&[sample].into())[0];
    effect_helper::mix(sample, processed, mix)
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
    "delay"
  }
}
