use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use fundsp::prelude32::{lowpass_hz, An, FixedSvf, LowpassMode};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_input_observer::EffectInputObserver;
use crate::control_input::ControlInputObserver;
use crate::error::Error;

pub struct LowPassFilterEffect {
  frequency: Arc<AtomicU16>,
  q_factor: Arc<AtomicU16>,
  filter: An<FixedSvf<f32, LowpassMode<f32>>>,
  low_pass_filter_id: String,
}

static LOW_PASS_FILTER_EFFECT_INCREMENTAL_ID: AtomicU16 = AtomicU16::new(0);

impl AudioEffect for LowPassFilterEffect {
  fn new() -> Self
  where
      Self: Sized
  {
    let low_pass_filter_id = format!("low_pass_filter_{}", LOW_PASS_FILTER_EFFECT_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));
    
    Self {
      frequency: Arc::new(AtomicU16::new(3273)), // 1000 Hz
      q_factor: Arc::new(AtomicU16::new(u16::MAX / 2)), // 0.707
      filter: lowpass_hz(1000.0, 0.707),
      low_pass_filter_id,
    }
  }

  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]> {
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

    self.filter.set_cutoff(frequency);
    self.filter.set_q(q_factor);

    chunk
        .into_iter()
        .map(|sample| {
          self.filter.tick(&[sample].into())[0]
        })
        .collect::<Vec<f32>>()
        .into_boxed_slice()
  }

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    match key {
      "frequency" => self.frequency.store(value, Ordering::Relaxed),
      "q_factor" => self.q_factor.store(value, Ordering::Relaxed),
      _ => return Err(Error::new("Unknown parameter")),
    };
    Ok(())
  }

  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error> {
    match key {
      "frequency" => {
        let observer = Arc::new(EffectInputObserver {
          value_storage: Arc::clone(&self.frequency),
        });
        Ok(observer)
      },
      "q_factor" => {
        let observer = Arc::new(EffectInputObserver {
          value_storage: Arc::clone(&self.q_factor),
        });
        Ok(observer)
      },
      _ => Err(Error::new("Parameter not found")),
    }
  }

  fn id(&self) -> &str {
    self.low_pass_filter_id.as_str()
  }
}
