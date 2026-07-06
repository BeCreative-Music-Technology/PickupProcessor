use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use fundsp::prelude32::{lowpass_hz, An, FixedSvf, LowpassMode};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_input_observer::EffectInputObserver;
use crate::control_input::ControlInputObserver;
use crate::error::Error;
use crate::logger;

pub struct LowPassFilterEffect {
  mix: Arc<AtomicU16>,
  frequency: Arc<AtomicU16>,
  q_factor: Arc<AtomicU16>,
  filter: An<FixedSvf<f32, LowpassMode<f32>>>,
}

static LOG_ENVIRONMENT: &str = "LowPassFilterEffect";

impl LowPassFilterEffect {
  pub fn new(mix: u16, frequency: u16, q_factor: u16) -> Self
  where
    Self: Sized
  {
    logger::info(LOG_ENVIRONMENT, "effect created");

    let frequency_f32 = Self::parse_frequency(frequency);
    let q_factor_f32 = Self::parse_q_factor(q_factor);

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      frequency: Arc::new(AtomicU16::new(frequency)), // 3273 // 1000 Hz
      q_factor: Arc::new(AtomicU16::new(q_factor)), // u16::MAX / 2 // 0.707
      filter: lowpass_hz(frequency_f32, q_factor_f32),
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
    if q_factor < u16::MAX / 2 {
      effect_helper::map(
        q_factor,
        u16::MIN,
        u16::MAX / 2,
        0.3,
        0.707
      )
    } else {
      effect_helper::map(
        q_factor,
        u16::MAX / 2,
        u16::MAX,
        0.707,
        10.0
      )
    }
  }
}

impl AudioEffect for LowPassFilterEffect {
  fn process_chunk(&mut self, sample: f32) -> f32 {
    let mix = effect_helper::map(
      self.mix.load(Ordering::Relaxed),
      u16::MIN,
      u16::MAX,
      0.0,
      1.0
    );
    let frequency = Self::parse_frequency(self.frequency.load(Ordering::Relaxed));
    let q_factor = Self::parse_q_factor(self.q_factor.load(Ordering::Relaxed));

    self.filter.set_cutoff_q(frequency, q_factor);
    
    let processed = self.filter.tick(&[sample].into())[0];
    effect_helper::mix(sample, processed, mix)
  }

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    match key {
      "mix" => self.mix.store(value, Ordering::Relaxed),
      "frequency" => self.frequency.store(value, Ordering::Relaxed),
      "q_factor" => self.q_factor.store(value, Ordering::Relaxed),
      _ => return Err(Error::new("Unknown parameter")),
    };
    logger::info(LOG_ENVIRONMENT, &format!("set parameter {} to {}", key, value));
    Ok(())
  }

  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error> {
    let value = match key {
      "mix" => &self.mix,
      "frequency" => &self.frequency,
      "q_factor" => &self.q_factor,
      _ => return Err(Error::new("Parameter not found")),
    };
    Ok(Arc::new(EffectInputObserver {
      value_storage: Arc::clone(value),
    }))
  }

  fn get_type(&self) -> &str {
    "low_pass_filter"
  }
}
