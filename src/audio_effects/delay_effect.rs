use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use fundsp::prelude32::{delay, An, Delay};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_input_observer::EffectInputObserver;
use crate::control_input::ControlInputObserver;
use crate::error::Error;
use crate::logger;

static LOG_ENVIRONMENT: &str = "LowPassFilterEffect";

pub struct DelayEffect {
  mix: Arc<AtomicU16>,
  delay: An<Delay>,
}

impl DelayEffect {
  pub fn new(mix: u16, delay_value: u16) -> Self
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

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      delay: delay(delay_value),
    }
  }
}

impl AudioEffect for DelayEffect {
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
        let processed =self.delay.tick(&[sample].into())[0];
        effect_helper::mix(sample, processed, mix)
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
    "delay"
  }
}
