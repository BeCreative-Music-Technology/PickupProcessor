use std::sync::Arc;
use crate::audio_effects::audio_effect::AudioEffect;
use crate::control_input::ControlInputObserver;
use crate::error::Error;

struct HardClipperEffect {
  
}

impl AudioEffect for HardClipperEffect {
  fn process_chunk(&mut self, chunk: f32) -> f32 {
    todo!()
  }

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    todo!()
  }

  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error> {
    todo!()
  }

  fn get_type(&self) -> &str {
    todo!()
  }
}
