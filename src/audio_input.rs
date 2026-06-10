use rtrb::Producer;
use crate::error::Error;

pub trait AudioInput: Send + Sync {
  fn open_stream(
    input_name: &str,
    producer: Producer<f32>
  ) -> Result<Self, Error> where Self: Sized;
  fn close_stream(&mut self);
  fn id(&self) -> &str;
}
