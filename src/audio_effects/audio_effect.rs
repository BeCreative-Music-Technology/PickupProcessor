use std::sync::Arc;
use crate::control_input::{ControlInputObserver};
use crate::error::Error;


pub trait AudioEffect: Send {
  fn process_chunk(&mut self, sample: f32) -> f32;
  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error>;
  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error>;
  fn get_type(&self) -> &str;
}
