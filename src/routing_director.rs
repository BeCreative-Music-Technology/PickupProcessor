use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use rtrb::{Consumer, Producer, RingBuffer};
use crate::audio_bus::AudioBus;
use crate::audio_input::AudioInput;
use crate::auxiliary_input::AuxiliaryInput;
use crate::error::Error;
use crate::logger;

static LOG_ENVIRONMENT: &str = "RoutingDirector";

/// Ring buffers hold this many JACK periods, so a late consumer does not immediately cause dropouts.
const RING_BUFFER_PERIODS: usize = 4;

pub struct RoutingDirector {
  audio_input: (Box<dyn AudioInput>, Consumer<f32>),
  audio_buses: Vec<(AudioBus, Producer<f32>)>,
  buffer_length: usize,
  main_volume: Arc<AtomicU16>,
}

impl RoutingDirector {
  ///
  /// Creates a new instance of `RoutingDirector`.
  /// This director is responsible for connecting the `AudioInput` with the instances of `AudioBus`.
  /// The `RoutingDirector` has an internal ring buffer on which the `AudioInput` writes captured data.
  ///
  /// Important: To update the director, `update()` must be called in a loop!
  ///
  /// `audio_input_name` takes an `&str` with the id of the audio output.
  /// An example of this could be `"system:capture_1"`.
  ///
  /// `buffer_length` takes a `usize` with the buffer length (JACK period) used to size the ring buffer(s).
  ///
  /// Example:
  /// ```ignore
  /// let mut routing_director = RoutingDirector::new("system:capture_1", 1024).unwrap();
  /// routing_director.add_audio_bus("system:playback_1").unwrap();
  /// routing_director.enable_audio_bus("bus_0").unwrap();
  ///
  /// loop {
  ///   routing_director.update();
  /// }
  /// ```
  ///
  pub fn new(audio_input_name: &str, buffer_length: usize) -> Result<RoutingDirector, Error> {
    let (input_producer, input_consumer) = RingBuffer::<f32>::new(buffer_length * RING_BUFFER_PERIODS);

    let audio_input = AuxiliaryInput::open_stream(audio_input_name, input_producer)?;

    logger::info(LOG_ENVIRONMENT, &format!("created, sample rate {} Hz", audio_input.sample_rate()));

    Ok(RoutingDirector {
      audio_input: (Box::new(audio_input), input_consumer),
      audio_buses: Vec::new(),
      buffer_length,
      main_volume: Arc::new(AtomicU16::new(u16::MAX)),
    })
  }

  ///
  /// `update()` must be called repeatedly by the owner.
  /// This method consumes all available data from the `AudioInput` and sends it to the enabled instances of `AudioBus`.
  ///
  /// Returns the number of samples that were routed, `0` means there was nothing to do.
  ///
  pub fn update(&mut self) -> usize {
    let input_consumer = &mut self.audio_input.1;
    let available = input_consumer.slots();
    if available == 0 {
      return 0;
    }

    let Ok(chunk) = input_consumer.read_chunk(available) else {
      return 0;
    };
    let (first, second) = chunk.as_slices();

    // Push the audio to the enabled audio buses, dropping what does not fit if a bus falls behind
    self.audio_buses
        .iter_mut()
        .filter(|(bus, _)| bus.is_enabled())
        .for_each(|(_, producer)| {
          let writable = producer.slots().min(available);
          if let Ok(output) = producer.write_chunk_uninit(writable) {
            output.fill_from_iter(first.iter().chain(second).copied());
          }
        });

    chunk.commit_all();
    available
  }

  ///
  /// Creates a new instance of `AudioBus`.
  /// The instance is enabled by default, but can be disabled using `disable_audio_bus()`.
  /// Upon creation, a ring buffer will be created that the bus uses to consume the captured data from.
  ///
  /// `audio_output_name` takes an `&str` with the id of the audio output.
  /// An example of this could be `"system:playback_1"`.
  ///
  pub fn add_audio_bus(&mut self, audio_output_name: &str) -> Result<(), Error> {
    let ring_buffer_length = self.buffer_length * RING_BUFFER_PERIODS;
    let (bus_producer, bus_consumer) = RingBuffer::<f32>::new(ring_buffer_length);
    let new_bus = AudioBus::new(
      bus_consumer,
      audio_output_name,
      true,
      ring_buffer_length,
      Arc::clone(&self.main_volume),
      self.sample_rate(),
    )?;

    self.audio_buses.push((
      new_bus,
      bus_producer,
    ));

    Ok(())
  }

  ///
  /// Enables the audio bus by its incremental id.
  ///
  /// `bus_id` is the incremental id of the `AudioBus` to be enabled, for example "bus_1".
  ///
  pub fn enable_audio_bus(&mut self, bus_id: &str) -> Result<(), Error> {
    match self.audio_bus(bus_id) {
      Some(bus) => {
        bus.enable();
        Ok(())
      }
      None => Err(Error::new(&format!("AudioBus {} not found", bus_id))),
    }
  }

  ///
  /// Disables the audio bus by its incremental id.
  ///
  /// `bus_id` is the incremental id of the `AudioBus` to be disabled, for example "bus_1".
  ///
  pub fn disable_audio_bus(&mut self, bus_id: &str) -> Result<(), Error> {
    match self.audio_bus(bus_id) {
      Some(bus) => {
        bus.disable();
        Ok(())
      }
      None => Err(Error::new(&format!("AudioBus {} not found", bus_id))),
    }
  }

  ///
  /// Returns the `AudioBus` with the given incremental id, for example "bus_1".
  ///
  pub fn audio_bus(&mut self, bus_id: &str) -> Option<&mut AudioBus> {
    self.audio_buses
      .iter_mut()
      .map(|(bus, _)| bus)
      .find(|bus| bus.id() == bus_id)
  }

  ///
  /// Sets the volume applied to every `AudioBus` after its effect chain.
  /// `0` is silent, `u16::MAX` is unity gain.
  ///
  pub fn set_main_volume(&self, volume: u16) {
    self.main_volume.store(volume, Ordering::Relaxed);
  }

  ///
  /// Returns the sample rate of the audio input in Hz.
  ///
  pub fn sample_rate(&self) -> usize {
    self.audio_input.0.sample_rate()
  }
}
