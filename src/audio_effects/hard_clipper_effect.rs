use std::sync::Arc;
use crate::audio_effects::audio_effect::AudioEffect;
use crate::control_input::ControlInputObserver;
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

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    Err(Error::new("Unknown parameter"))
  }

  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error> {
    Err(Error::new("Parameter not found"))
  }

  fn get_type(&self) -> &str {
    "hard_clipper"
  }
}
