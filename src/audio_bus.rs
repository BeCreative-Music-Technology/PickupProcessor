use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use rtrb::{Consumer, RingBuffer};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::hard_clipper_effect::HardClipperEffect;
use crate::audio_output::AudioOutput;
use crate::auxiliary_output::AuxiliaryOutput;
use crate::error::Error;
use crate::logger;

pub struct AudioBus {
  enabled: Arc<AtomicBool>,
  audio_output: Box<dyn AudioOutput>,
  effects: Arc<Mutex<Vec<Box<dyn AudioEffect>>>>,
  bus_id: String,
  thread: JoinHandle<()>,
}

static BUS_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);
static LOG_ENVIRONMENT: &str = "AudioBus";

/// How long the processing thread sleeps when there is no audio to process.
pub const IDLE_SLEEP: Duration = Duration::from_micros(250);

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
  /// `main_volume` takes a shared `u16` volume (`0` is silent, `u16::MAX` is unity gain)
  /// that is applied after the effect chain.
  ///
  pub fn new(
    mut consumer: Consumer<f32>,
    audio_output_name: &str,
    enabled: bool,
    buffer_length: usize,
    main_volume: Arc<AtomicU16>,
  ) -> Result<Self, Error> {
    let bus_id = format!("bus_{}", BUS_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));

    // Create ring buffer for transferring audio to output instance
    let (mut output_producer, output_consumer) = RingBuffer::<f32>::new(buffer_length);

    // Create new instance of AuxiliaryOutput stream
    let audio_output = AuxiliaryOutput::open_stream(audio_output_name, output_consumer)?;

    // Clone pointers
    let effects = Arc::new(Mutex::new(Vec::<Box<dyn AudioEffect>>::new()));
    let thread_effects = effects.clone();

    let atomic_enabled = Arc::new(AtomicBool::new(enabled));
    let thread_enabled = Arc::clone(&atomic_enabled);

    // Create a thread for processing the incoming audio
    let handle = thread::spawn(move || {
      let mut hard_clipper = HardClipperEffect::new(-1.0, 1.0);

      loop {
        if !thread_enabled.load(Ordering::Acquire) {
          thread::park();
          continue;
        }

        // Process everything that is available in one go, so the effects lock is taken once per chunk
        let available = consumer.slots().min(output_producer.slots());
        if available == 0 {
          thread::sleep(IDLE_SLEEP);
          continue;
        }

        let (Ok(input), Ok(output)) = (consumer.read_chunk(available), output_producer.write_chunk_uninit(available)) else {
          continue;
        };

        let volume = main_volume.load(Ordering::Relaxed) as f32 / u16::MAX as f32;
        let mut effects = thread_effects.lock().unwrap();
        output.fill_from_iter(input.into_iter().map(|sample| {
          let processed = effects
            .iter_mut()
            .fold(sample, |audio, effect| effect.process_chunk(audio));
          hard_clipper.process_chunk(processed * volume)
        }));
      }
    });

    logger::info(LOG_ENVIRONMENT, &format!("{} created", bus_id));

    Ok(Self {
      enabled: atomic_enabled,
      audio_output: Box::new(audio_output),
      effects,
      bus_id,
      thread: handle,
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
  /// Replaces the whole effect chain at once, so the audio thread never processes a half-built chain.
  /// The old effects are dropped after the lock is released to keep the audio thread blocked as short as possible.
  ///
  pub fn set_effects(&mut self, effects: Vec<Box<dyn AudioEffect>>) {
    let types: Vec<&str> = effects.iter().map(|effect| effect.get_type()).collect();
    logger::info(LOG_ENVIRONMENT, &format!("{} effect chain set to [{}]", self.bus_id, types.join(", ")));

    let old_effects = std::mem::replace(&mut *self.effects.lock().unwrap(), effects);
    drop(old_effects);
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
    if self.enabled.swap(true, Ordering::Release) {
      return;
    }
    logger::info(LOG_ENVIRONMENT, &format!("{} enabled", self.bus_id));
    // Unpark after storing the flag, otherwise the thread can wake, still see `false` and park forever
    self.thread.thread().unpark();
  }

  ///
  /// Disables the `AudioBus` so it stops processing the incoming data.
  ///
  pub fn disable(&mut self) {
    if !self.enabled.swap(false, Ordering::Release) {
      return;
    }
    logger::info(LOG_ENVIRONMENT, &format!("{} disabled", self.bus_id));
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
