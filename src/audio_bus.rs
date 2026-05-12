use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use ringbuf::{CachingCons, CachingProd, HeapRb, SharedRb};
use ringbuf::storage::Heap;
use ringbuf::traits::Split;
use crate::audio_output::AudioOutput;
use crate::auxiliary_output::AuxiliaryOutput;
use crate::error::Error;

pub struct AudioBus {
  consumer: CachingCons<Arc<SharedRb<Heap<f32>>>>,
  audio_output: (Box<dyn AudioOutput>, CachingProd<Arc<SharedRb<Heap<f32>>>>),
  bus_id: String,
}

static BUS_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);

impl AudioBus {
  pub fn new(consumer: CachingCons<Arc<SharedRb<Heap<f32>>>>, audio_output_name: &str) -> Result<Self, Error> {
    let bus_id = format!("bus_{}", BUS_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));

    // Create ring buffer for transferring audio to output instance
    let output_ring_buffer = HeapRb::<f32>::new(2048);
    let (output_producer, output_consumer) = output_ring_buffer.split();

    // Create new instance of AuxiliaryOutput stream
    let audio_output = match AuxiliaryOutput::open_stream(
      audio_output_name,
      output_consumer
    ) {
      Ok(audio_output) => audio_output,
      Err(e) => return Err(e),
    };

    Ok(Self {
      consumer,
      audio_output: (Box::new(audio_output), output_producer),
      bus_id,
    })
  }
  
  pub fn id(&self) -> &str {
    self.bus_id.as_str()
  }
}
