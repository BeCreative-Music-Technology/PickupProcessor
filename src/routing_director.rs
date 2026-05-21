use rtrb::{Consumer, Producer, RingBuffer};
use crate::audio_bus::AudioBus;
use crate::audio_input::AudioInput;
use crate::auxiliary_input::AuxiliaryInput;
use crate::error::Error;

pub struct RoutingDirector {
  audio_input: (Box<dyn AudioInput>, Consumer<f32>),
  audio_buses: Vec<(AudioBus, Producer<f32>)>,
  buffer_length: usize,
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
  /// `buffer_length` takes a `usize` with the buffer length used by the ring buffer(s).
  ///
  /// Example:
  /// ```
  /// fn main() {
  ///   let routing_director = RoutingDirector::new("system:capture_1", 1024).unwrap();
  ///   routing_director.add_audio_bus("system:playback_1").unwrap();
  ///   routing_director.enable_audio_bus("bus_1");
  ///
  ///   loop {
  ///     routing_director.update();
  ///   }
  /// }
  /// ```
  ///
  pub fn new(audio_input_name: &str, buffer_length: usize) -> Result<RoutingDirector, Error> {
    let (input_producer, input_consumer) = RingBuffer::<f32>::new(buffer_length);

    let audio_input = match AuxiliaryInput::open_stream(
      audio_input_name,
      input_producer,
    ) {
      Ok(audio_input) => audio_input,
      Err(e) => return Err(e),
    };

    let audio_buses = Vec::new();

    Ok(RoutingDirector {
      audio_input: (Box::new(audio_input), input_consumer),
      audio_buses,
      buffer_length
    })
  }

  ///
  /// `update()` must be called repeatedly by the owner.
  /// This method consumes the data from the `AudioInput` and sends it to the enabled instances of `AudioBus`.
  ///
  pub fn update(&mut self) {
    // Get audio slice from input
    let input_consumer = &mut self.audio_input.1;
    let new_sample = match input_consumer.pop() {
      Ok(new_sample) => new_sample,
      Err(_) => return,
    };

    // Collect enabled audio buses and push the audio slice
    self.audio_buses
        .iter_mut()
        .filter(|bus| bus.0.is_enabled() == true)
        .for_each(|bus| _ = bus.1.push(new_sample));
  }

  ///
  /// Creates a new instance of `AudioBus`.
  /// The instance is disabled by default, but can be enabled using `enable_audio_bus()`.
  /// Upon creation, a ring buffer will be created that the bus uses to consume the captured data from.
  ///
  /// `audio_output_name` takes an `&str` with the id of the audio output.
  /// An example of this could be `"system:playback_1"`.
  ///
  pub fn add_audio_bus(&mut self, audio_output_name: &str) -> Result<(), Error> {
    let (bus_producer, bus_consumer) = RingBuffer::<f32>::new(2048);
    let new_bus = match AudioBus::new(bus_consumer, audio_output_name, false, self.buffer_length) {
      Ok(new_bus) => new_bus,
      Err(e) => return Err(e),
    };

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
    if let Some(bus) = self.audio_buses
        .iter_mut()
        .find(|bus| bus.0.id() == bus_id) {
          bus.0.enable();
          Ok(())
        } 
    else {
      Err(Error {
        message: format!("AudioBus {} not found", bus_id),
      })
    }
  }

  ///
  /// Disables the audio bus by its incremental id.
  ///
  /// `bus_id` is the incremental id of the `AudioBus` to be disabled, for example "bus_1".
  ///
  pub fn disable_audio_bus(&mut self, bus_id: &str) -> Result<(), Error> {
    if let Some(bus) = self.audio_buses
        .iter_mut()
        .find(|bus| bus.0.id() == bus_id) {
      bus.0.disable();
      Ok(())
    }
    else {
      Err(Error {
        message: format!("AudioBus {} not found", bus_id),
      })
    }
  }

  ///
  /// Returns all existing instances of `AudioBus` as a `Vec<&AudioBus>`.
  ///
  pub fn audio_buses(&self) -> Vec<&AudioBus> {
    self.audio_buses.iter().map(|(bus, _)| bus).collect()
  }
}
