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

impl AudioEffect for LowPassFilterEffect {
  fn new(mix: u16) -> Self
  where
      Self: Sized
  {
    logger::info(LOG_ENVIRONMENT, "effect created");

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      frequency: Arc::new(AtomicU16::new(3273)), // 1000 Hz
      q_factor: Arc::new(AtomicU16::new(u16::MAX / 2)), // 0.707
      filter: lowpass_hz(1000.0, 0.707),
    }
  }

  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]> {
    let mix = effect_helper::map(
      self.mix.load(Ordering::Relaxed),
      u16::MIN,
      u16::MAX,
      0.0,
      1.0
    );
    let frequency = effect_helper::map(
      self.frequency.load(Ordering::Relaxed),
      u16::MIN,
      u16::MAX,
      20.0,
      20000.0
    );
    let q_factor_u16 = self.q_factor.load(Ordering::Relaxed);
    let q_factor = if q_factor_u16 < u16::MAX / 2 {
      effect_helper::map(
        q_factor_u16,
        u16::MIN,
        u16::MAX / 2,
        0.3,
        0.707
      )
    } else {
      effect_helper::map(
        q_factor_u16,
        u16::MAX / 2,
        u16::MAX,
        0.707,
        10.0
      )
    };

    self.filter.set_cutoff_q(frequency, q_factor);

    chunk
        .into_iter()
        .map(|sample| {
          let processed = self.filter.tick(&[sample].into())[0];
          effect_helper::mix(sample, processed, mix)
        })
        .collect::<Vec<f32>>()
        .into_boxed_slice()
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
