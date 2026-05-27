use crate::error::Error;

pub trait AudioEffect: Send {
  fn new() -> Self where Self: Sized;
  fn process_chunk(&self, chunk: Vec<f32>) -> Box<[f32]>;
  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error>;
}
