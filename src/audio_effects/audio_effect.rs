use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use crate::control_input::{ControlInputObserver, ObservableControlInput};
use crate::error::Error;

pub trait AudioEffect: Send + Sync {
  fn new() -> Self where Self: Sized;
  fn process_chunk(&self, chunk: Vec<f32>) -> Box<[f32]>;
  ///
  /// Sets the new value of the gain effect.
  /// `u16: 0` is -6db, `u16: 327675` is +0db, `u16: 65,535` is +18db.
  ///
  /// `key` takes a `&str` with the key of the parameter to be changed, for example `"gain"`.
  ///
  /// `value` takes an 'u16' with the new value.
  ///
  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error>;
  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error>;
}
