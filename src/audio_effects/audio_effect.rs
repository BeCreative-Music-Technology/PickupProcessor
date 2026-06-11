use std::sync::Arc;
use crate::control_input::{ControlInputObserver};
use crate::error::Error;

pub trait AudioEffect: Send {
  fn new() -> Self where Self: Sized;
  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]>;
  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error>;
  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error>;
  fn get_type(&self) -> &str;
}
