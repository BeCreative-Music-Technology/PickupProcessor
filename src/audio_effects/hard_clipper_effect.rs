use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use crate::audio_effects::audio_effect::AudioEffect;
use crate::error::Error;
use crate::logger;

pub struct HardClipperEffect {
  min: f32,
  max: f32,
}

static LOG_ENVIRONMENT: &str = "HardClipperEffect";

impl HardClipperEffect {
  pub fn new(min: f32, max: f32) -> Self {
    logger::info(LOG_ENVIRONMENT, "effect created");
    
    Self { min, max }
  }
}

impl AudioEffect for HardClipperEffect {
  fn process_chunk(&mut self, sample: f32) -> f32 {
    sample.clamp(self.min, self.max)
  }

  fn set_value(&mut self, _key: &str, _value: u16) -> Result<(), Error> {
    Err(Error::new("Unknown parameter"))
  }

  fn parameter(&self, _key: &str) -> Result<Arc<AtomicU16>, Error> {
    Err(Error::new("Parameter not found"))
  }

  fn get_type(&self) -> &str {
    "hard_clipper"
  }
}
