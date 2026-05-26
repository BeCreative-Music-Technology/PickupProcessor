use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread;
use std::thread::JoinHandle;
use rtrb::{Consumer, RingBuffer};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_output::AudioOutput;
use crate::auxiliary_output::AuxiliaryOutput;
use crate::error::Error;

pub struct AudioBus {
  enabled: Arc<AtomicBool>,
  audio_output: Box<dyn AudioOutput>,
  effects: Arc<Mutex<Vec<Box<dyn AudioEffect>>>>,
  effect_buffer: Arc<Mutex<Vec<f32>>>,
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

    // Clone pointers
    let effects = Arc::new(Mutex::new(Vec::<Box<dyn AudioEffect>>::new()));
    let thread_effects = effects.clone();

    let effect_buffer = Arc::new(Mutex::new(Vec::new()));
    let thread_effect_buffer = effect_buffer.clone();

    let atomic_enabled = Arc::new(AtomicBool::new(enabled));
    let thread_enabled = Arc::clone(&atomic_enabled);

    // Create a thread for processing the incoming audio
    let handle = thread::spawn(move || while thread_enabled.load(Ordering::Relaxed) {
      let incoming_audio = match consumer.pop() {
        Ok(incoming_audio) => incoming_audio,
        Err(_) => continue,
      };

      // Push incoming audio to effect buffer and continue loop when buffer is not full yet
      let mut effect_buffer = thread_effect_buffer.lock().unwrap();
      effect_buffer.push(incoming_audio);
      if effect_buffer.len() < buffer_length { continue; }
      
      // Clone, empty and process data from effect buffer
      let mut processed_audio = effect_buffer.clone();
      effect_buffer.clear();
      for effect in thread_effects.lock().unwrap().iter() {
        processed_audio = effect
            .process_chunk(processed_audio)
            .into_vec();
      }

      _ = output_producer.push_partial_slice(&processed_audio);
    });

    Ok(Self {
      enabled: atomic_enabled,
      audio_output: Box::new(audio_output),
      effects,
      effect_buffer,
      bus_id,
      thread: Some(handle),
    })
  }

  ///
  /// Adds new effect to the effect chain
  ///
  pub fn add_effect(&mut self, effect: Box<dyn AudioEffect>) {
    self.effects.lock().unwrap().push(effect);
  }

  ///
  /// Removes effect from the effect chain
  ///
  pub fn remove_effect(&mut self, index: usize) {
    self.effects.lock().unwrap().remove(index);
  }

  ///
  /// Enables the `AudioBus` so it starts processing the incoming data.
  ///
  pub fn enable(&mut self) {
    self.enabled.store(true, Ordering::Relaxed);
  }

  ///
  /// Disables the `AudioBus` so it stops processing the incoming data.
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
