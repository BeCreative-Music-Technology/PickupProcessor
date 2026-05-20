use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread;
use std::thread::JoinHandle;
use rtrb::{Consumer, RingBuffer};
use crate::audio_output::AudioOutput;
use crate::auxiliary_output::AuxiliaryOutput;
use crate::error::Error;

pub struct AudioBus {
  enabled: Arc<AtomicBool>,
  audio_output: Box<dyn AudioOutput>,
  bus_id: String,
  thread: Option<JoinHandle<()>>
}

static BUS_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);

impl AudioBus {
  pub fn new(mut consumer: Consumer<f32>, audio_output_name: &str, enabled: bool, buffer_length: usize) -> Result<Self, Error> {
    let bus_id = format!("bus_{}", BUS_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));

    // Create ring buffer for transferring audio to output instance
    let (mut output_producer, output_consumer) = RingBuffer::<f32>::new(buffer_length);

    // Create new instance of AuxiliaryOutput stream
    let audio_output = match AuxiliaryOutput::open_stream(
      audio_output_name,
      output_consumer
    ) {
      Ok(audio_output) => audio_output,
      Err(e) => return Err(e),
    };

    let atomic_enabled = Arc::new(AtomicBool::new(enabled));
    let thread_enabled = Arc::clone(&atomic_enabled);

    let handle = thread::spawn(move || {
      while thread_enabled.load(Ordering::Relaxed) {
        let incoming_audio = match consumer.pop() {
          Ok(incoming_audio) => incoming_audio,
          Err(_) => continue,
        };

        // TODO: Apply audio effects
        let processed_audio = incoming_audio;

        _ = output_producer.push(processed_audio);
      }
    });

    Ok(Self {
      enabled: atomic_enabled,
      audio_output: Box::new(audio_output),
      bus_id,
      thread: Some(handle),
    })
  }

  pub fn enable(&mut self) {
    self.enabled.store(true, Ordering::Relaxed);
  }

  pub fn disable(&mut self) {
    self.enabled.store(false, Ordering::Relaxed);
  }

  pub fn is_enabled(&self) -> bool {
    self.enabled.load(Ordering::Relaxed)
  }
  
  pub fn id(&self) -> &str {
    self.bus_id.as_str()
  }
}
