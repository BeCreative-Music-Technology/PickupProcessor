use std::sync::Arc;
use ringbuf::{CachingCons, SharedRb};
use ringbuf::storage::Heap;
use crate::error::Error;

pub trait AudioOutput {
  fn open_stream(
    output_name: &str,
    consumer: CachingCons<Arc<SharedRb<Heap<f32>>>>
  ) -> Result<Self, Error> where Self: Sized;
  fn close_stream(&mut self);
  fn id(&self) -> &str;
}