use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use crate::error::Error;


pub trait AudioEffect: Send {
  fn process_chunk(&mut self, sample: f32) -> f32;
  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error>;
  /// Shared storage of a parameter that can be changed live, e.g. by a control input.
  fn parameter(&self, key: &str) -> Result<Arc<AtomicU16>, Error>;
  fn get_type(&self) -> &str;
}
