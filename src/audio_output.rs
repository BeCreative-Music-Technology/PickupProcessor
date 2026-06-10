use rtrb::Consumer;
use crate::error::Error;

pub trait AudioOutput: Send + Sync {
  fn open_stream(
    output_name: &str,
    consumer: Consumer<f32>
  ) -> Result<Self, Error> where Self: Sized;
  fn close_stream(&mut self);
  fn id(&self) -> &str;
}