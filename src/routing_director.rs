use std::sync::Arc;
use ringbuf::{CachingCons, CachingProd, HeapRb, SharedRb};
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Producer, Split};
use crate::audio_bus::AudioBus;
use crate::audio_input::AudioInput;
use crate::auxiliary_input::AuxiliaryInput;
use crate::error::Error;

pub struct RoutingDirector {
  audio_input: (Box<dyn AudioInput>, CachingCons<Arc<SharedRb<Heap<f32>>>>),
  audio_buses: Vec<(AudioBus, CachingProd<Arc<SharedRb<Heap<f32>>>>)>,
}

impl RoutingDirector {
  pub fn new(audio_input_name: &str) -> Result<RoutingDirector, Error> {
    let input_ring_buffer = HeapRb::<f32>::new(2048);
    let (input_producer, input_consumer) = input_ring_buffer.split();

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
      audio_buses
    })
  }

  pub fn update(&mut self) {
    // Get audio slice from input
    let input_consumer = &mut self.audio_input.1;
    let new_slice = input_consumer.try_pop().unwrap_or(0.0);

    // Collect enabled audio buses and push the audio slice
    self.audio_buses
        .iter_mut()
        .filter(|bus| bus.0.is_enabled() == true)
        .for_each(|bus| _ = bus.1.try_push(new_slice));
  }

  pub fn add_audio_bus(&mut self, audio_output_name: &str) -> Result<(), Error> {
    let bus_ring_buffer = HeapRb::<f32>::new(2048);
    let (bus_producer, bus_consumer) = bus_ring_buffer.split();
    let new_bus = match AudioBus::new(bus_consumer, audio_output_name, false) {
      Ok(new_bus) => new_bus,
      Err(e) => return Err(e),
    };

    self.audio_buses.push((
      new_bus,
      bus_producer,
    ));
    
    Ok(())
  }

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

  pub fn audio_buses(&self) -> Vec<&AudioBus> {
    self.audio_buses.iter().map(|(bus, _)| bus).collect()
  }
}
