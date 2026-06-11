use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread;
use std::thread::JoinHandle;
use rtrb::{Consumer, RingBuffer};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_output::AudioOutput;
use crate::auxiliary_output::AuxiliaryOutput;
use crate::error::Error;
use crate::logger;

pub struct AudioBus {
  enabled: Arc<AtomicBool>,
  audio_output: Box<dyn AudioOutput>,
  effects: Arc<Mutex<Vec<Box<dyn AudioEffect>>>>,
  effect_buffer: Arc<Mutex<Vec<f32>>>,
  bus_id: String,
  thread: Option<JoinHandle<()>>
}

static BUS_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);
static LOG_ENVIRONMENT: &str = "AudioBus";

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

      // Clone, empty and process data from effect buffer
      let mut processed_audio = {
        let mut effect_buffer = thread_effect_buffer.lock().unwrap();
        effect_buffer.push(incoming_audio);

        if effect_buffer.len() < buffer_length {
          continue;
        }

        let data = effect_buffer.clone();
        effect_buffer.clear();
        data
      };
      for effect in thread_effects.lock().unwrap().iter_mut() {
        processed_audio = effect
            .process_chunk(processed_audio)
            .into_vec();
      }

      _ = output_producer.push_partial_slice(&processed_audio);
    });

    logger::info(LOG_ENVIRONMENT, &format!("{} created", &bus_id));

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
    logger::info(LOG_ENVIRONMENT, &format!("{} added to {}", effect.get_type(), self.bus_id));
    self.effects.lock().unwrap().push(effect);
  }

  ///
  /// Retrieves a copy of all effects from the effect chain
  ///
  pub fn effects(&self) -> Arc<Mutex<Vec<Box<dyn AudioEffect>>>> {
    self.effects.clone()
  }

  ///
  /// Removes effect from the effect chain
  ///
  pub fn remove_effect(&mut self, index: usize) {
    self.effects.lock().unwrap().remove(index);
  }

  ///
  /// Removes all effects from the effect chain
  ///
  pub fn clear_effects(&mut self) {
    logger::info(LOG_ENVIRONMENT, &format!("cleared effect chain for {}", self.bus_id));
    self.effects.lock().unwrap().clear();
  }

  ///
  /// High-order function for applying changes to effects.
  ///
  /// `index` takes a `usize` as the index of the effect to be changed.
  ///
  /// `f` takes function with a `&mut dyn AudioEffect` as a parameter.
  ///
  pub fn for_effect<F, R>(&mut self, index: usize, f: F) -> Result<R, Error>
  where
      F: FnOnce(&mut dyn AudioEffect) -> R
  {
    let mut effects = self.effects.lock().unwrap();

    match effects.get_mut(index) {
      Some(effect) => Ok(f(effect.as_mut())),
      None => Err(Error::new("Index out of bounds")),
    }
  }

  ///
  /// Enables the `AudioBus` so it starts processing the incoming data.
  ///
  pub fn enable(&mut self) {
    logger::info(LOG_ENVIRONMENT, &format!("{} enabled", &self.bus_id));
    self.enabled.store(true, Ordering::Relaxed);
  }

  ///
  /// Disables the `AudioBus` so it stops processing the incoming data.
  ///
  pub fn disable(&mut self) {
    logger::info(LOG_ENVIRONMENT, &format!("{} disabled", &self.bus_id));
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
