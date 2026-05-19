use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::thread::JoinHandle;
use ringbuf::{CachingCons, HeapRb, SharedRb};
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Producer, Split};
use crate::audio_output::AudioOutput;
use crate::auxiliary_output::AuxiliaryOutput;
use crate::error::Error;

pub struct AudioBus {
  enabled: bool,
  audio_output: Box<dyn AudioOutput>,
  bus_id: String,
  thread: Option<JoinHandle<()>>
}

static BUS_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);

impl AudioBus {
  pub fn new(mut consumer: CachingCons<Arc<SharedRb<Heap<f32>>>>, audio_output_name: &str, enabled: bool) -> Result<Self, Error> {
    let bus_id = format!("bus_{}", BUS_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));

    // Create ring buffer for transferring audio to output instance
    let output_ring_buffer = HeapRb::<f32>::new(2048);
    let (mut output_producer, output_consumer) = output_ring_buffer.split();

    // Create new instance of AuxiliaryOutput stream
    let audio_output = match AuxiliaryOutput::open_stream(
      audio_output_name,
      output_consumer
    ) {
      Ok(audio_output) => audio_output,
      Err(e) => return Err(e),
    };
    
    let handle = thread::spawn(move || {
      while enabled {
        let incoming_audio = consumer.try_pop().unwrap_or(0.0);

        // TODO: Apply audio effects
        let processed_audio = incoming_audio;

        _ = output_producer.try_push(processed_audio);
      }
    });

    Ok(Self {
      enabled,
      audio_output: Box::new(audio_output),
      bus_id,
      thread: Some(handle),
    })
  }

  pub fn enable(&mut self) {
    self.enabled = true;
  }

  pub fn disable(&mut self) {
    self.enabled = false;
  }

  pub fn is_enabled(&self) -> bool {
    self.enabled
  }
  
  pub fn id(&self) -> &str {
    self.bus_id.as_str()
  }
}
