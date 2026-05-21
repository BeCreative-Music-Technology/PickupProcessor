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
  ///
  /// Opens a new instance of `AudioBus`.
  /// To use or disable the bus, it should be controlled using the `enable()` and `disable()` methods.
  /// The `AudioBus` has an internal ring buffer which the `AudioOutput` gets its audio data from.
  /// This data is sequentially processed using effects.
  /// The audio processing for each `AudioBus` is done on a separate `std::thread`.
  ///
  /// `consumer` takes a reference to a mutable ring buffer consumer.
  /// The stream will be read from the ring buffer using it.
  ///
  /// `audio_output_name` takes an `&str` with the id of the audio output.
  /// An example of this could be `"system:playback_1"`.
  ///
  /// `enabled` takes a `bool` which enables the `AudioBus`.
  ///
  /// `buffer_length` takes a `usize` which sets the buffer size of the internal ring buffer.
  ///
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

    // Create a thread for processing the incoming audio
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

  ///
  /// Enables the `AudioBus` so it starts processing the incoming data.
  ///
  pub fn enable(&mut self) {
    self.enabled.store(true, Ordering::Relaxed);
  }

  ///
  /// Disables the `AudioBus` so it stops processing the incomming data.
  ///
  pub fn disable(&mut self) {
    self.enabled.store(false, Ordering::Relaxed);
  }

  ///
  /// Returns a `bool` based on the enabled status of the `AudioBus`.
  /// `true` means the bus is enabled.
  /// `false` means the bus is disabled.
  ///
  pub fn is_enabled(&self) -> bool {
    self.enabled.load(Ordering::Relaxed)
  }

  ///
  /// Returns the incremental id of the `AudioBus`.
  ///
  pub fn id(&self) -> &str {
    self.bus_id.as_str()
  }
}
