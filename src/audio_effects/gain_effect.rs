use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_helper::Smoothed;
use crate::error::Error;
use crate::logger;

pub struct GainEffect {
  mix: Arc<AtomicU16>,
  gain_value: Arc<AtomicU16>,
  // Cached (raw value, linear factor) so `powf` only runs when the gain changes
  gain_cache: (u16, f32),
  smoothed_gain: Smoothed,
  smoothed_mix: Smoothed,
}

static LOG_ENVIRONMENT: &str = "GainEffect";

impl GainEffect {
  pub fn new(mix: u16, gain: u16, sample_rate: f64) -> Self
  where
    Self: Sized
  {
    logger::info(LOG_ENVIRONMENT, "effect created");

    let gain_factor = Self::db_to_gain(Self::parse_gain(gain));
    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      gain_value: Arc::new(AtomicU16::new(gain)), // 16384 // 0 dB
      gain_cache: (gain, gain_factor),
      smoothed_gain: Smoothed::new(gain_factor, sample_rate),
      smoothed_mix: Smoothed::new(Self::parse_mix(mix), sample_rate),
    }
  }

  fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
  }

  fn parse_gain(gain: u16) -> f32 {
    effect_helper::map(gain, u16::MIN, u16::MAX, -6.0, 18.0) // -6 dB -> 18 dB
  }

  fn parse_mix(mix: u16) -> f32 {
    effect_helper::map(mix, u16::MIN, u16::MAX, 0.0, 1.0)
  }
}

impl AudioEffect for GainEffect {
  ///
  /// Processes the chunk of data given to the method and applies a gain.
  /// The gain is done by multiplying the signal with a set factor.
  /// Minimum gain value is -6db, highest gain value is +18db.
  ///
  /// `sample` takes an `f32` which contains the sample to be processed by the gain effect.
  ///
  fn process_chunk(&mut self, sample: f32) -> f32 {
    let gain = self.gain_value.load(Ordering::Relaxed);
    if gain != self.gain_cache.0 {
      self.gain_cache = (gain, Self::db_to_gain(Self::parse_gain(gain)));
    }

    let gain_factor = self.smoothed_gain.next(self.gain_cache.1);
    let mix = self.smoothed_mix.next(Self::parse_mix(self.mix.load(Ordering::Relaxed)));

    let processed = sample * gain_factor;
    effect_helper::mix(sample, processed, mix)
  }

  ///
  /// Sets the new value of the gain effect.
  /// `u16: 0` is -6db, `u16: 16384` is +0db, `u16: 65,535` is +18db.
  ///
  /// `key` takes a `&str` with the key of the parameter to be changed, for example `"gain"`.
  ///
  /// `value` takes an 'u16' with the new value.
  ///
  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    self.parameter(key)?.store(value, Ordering::Relaxed);
    logger::info(LOG_ENVIRONMENT, &format!("set parameter {} to {}", key, value));
    Ok(())
  }

  fn parameter(&self, key: &str) -> Result<Arc<AtomicU16>, Error> {
    match key {
      "mix" => Ok(self.mix.clone()),
      "gain" => Ok(self.gain_value.clone()),
      _ => Err(Error::new("Parameter not found")),
    }
  }

  fn get_type(&self) -> &str {
    "gain"
  }
}
