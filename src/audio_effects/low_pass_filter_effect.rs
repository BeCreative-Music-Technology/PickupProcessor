use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use fundsp::prelude32::{lowpass_hz, An, FixedSvf, LowpassMode};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_helper::Smoothed;
use crate::error::Error;
use crate::logger;

pub struct LowPassFilterEffect {
  mix: Arc<AtomicU16>,
  frequency: Arc<AtomicU16>,
  q_factor: Arc<AtomicU16>,
  filter: An<FixedSvf<f32, LowpassMode<f32>>>,
  smoothed_mix: Smoothed,
  smoothed_frequency: Smoothed,
  // Cutoff (Hz) and raw q_factor the filter coefficients were last calculated for
  applied: (f32, u16),
  samples_until_update: u32,
}

static LOG_ENVIRONMENT: &str = "LowPassFilterEffect";

impl LowPassFilterEffect {
  // Recalculating the coefficients is expensive, so a gliding cutoff is applied every few samples
  const COEFFICIENT_UPDATE_INTERVAL: u32 = 16;
  // Smaller cutoff changes than this are inaudible and not worth recalculating for
  const CUTOFF_TOLERANCE_HZ: f32 = 0.5;

  pub fn new(mix: u16, frequency: u16, q_factor: u16, sample_rate: f64) -> Self
  where
    Self: Sized
  {
    logger::info(LOG_ENVIRONMENT, "effect created");

    let frequency_f32 = Self::parse_frequency(frequency);
    let q_factor_f32 = Self::parse_q_factor(q_factor);

    let mut filter = lowpass_hz(frequency_f32, q_factor_f32);
    filter.set_sample_rate(sample_rate);

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      frequency: Arc::new(AtomicU16::new(frequency)), // 3273 // 1000 Hz
      q_factor: Arc::new(AtomicU16::new(q_factor)), // 2750 // 0.707
      filter,
      smoothed_mix: Smoothed::new(Self::parse_mix(mix), sample_rate),
      smoothed_frequency: Smoothed::new(frequency_f32, sample_rate),
      applied: (frequency_f32, q_factor),
      samples_until_update: 0,
    }
  }

  fn parse_frequency(frequency: u16) -> f32 {
    effect_helper::map(
      frequency,
      u16::MIN,
      u16::MAX,
      20.0,
      20000.0
    )
  }

  fn parse_q_factor(q_factor: u16) -> f32 {
    effect_helper::map(
      q_factor,
      u16::MIN,
      u16::MAX,
      0.3,
      10.0
    )
  }

  fn parse_mix(mix: u16) -> f32 {
    effect_helper::map(mix, u16::MIN, u16::MAX, 0.0, 1.0)
  }
}

impl AudioEffect for LowPassFilterEffect {
  fn process_chunk(&mut self, sample: f32) -> f32 {
    let mix = self.smoothed_mix.next(Self::parse_mix(self.mix.load(Ordering::Relaxed)));
    let frequency = self.smoothed_frequency.next(Self::parse_frequency(self.frequency.load(Ordering::Relaxed)));

    if self.samples_until_update == 0 {
      self.samples_until_update = Self::COEFFICIENT_UPDATE_INTERVAL;
      let q_factor = self.q_factor.load(Ordering::Relaxed);
      if (frequency - self.applied.0).abs() > Self::CUTOFF_TOLERANCE_HZ || q_factor != self.applied.1 {
        self.applied = (frequency, q_factor);
        self.filter.set_cutoff_q(frequency, Self::parse_q_factor(q_factor));
      }
    }
    self.samples_until_update -= 1;

    let processed = self.filter.tick(&[sample].into())[0];
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
      "frequency" => Ok(self.frequency.clone()),
      "q_factor" => Ok(self.q_factor.clone()),
      _ => Err(Error::new("Parameter not found")),
    }
  }

  fn get_type(&self) -> &str {
    "low_pass_filter"
  }
}
