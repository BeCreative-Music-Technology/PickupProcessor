use std::sync::Arc;
use ringbuf::{CachingProd, SharedRb};
use ringbuf::storage::Heap;
use crate::error::Error;

pub trait AudioInput {
  fn open_stream(
    input_name: &str,
    producer: CachingProd<Arc<SharedRb<Heap<f32>>>>
  ) -> Result<Self, Error> where Self: Sized;
  fn close_stream(&mut self);
  fn id(&self) -> &str;
}
