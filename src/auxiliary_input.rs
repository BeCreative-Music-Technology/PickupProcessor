use std::sync::Arc;
use jack::{AudioIn, Client, ClientOptions, Control, ProcessScope};
use jack::contrib::ClosureProcessHandler;
use ringbuf::{CachingProd, SharedRb};
use ringbuf::storage::Heap;
use ringbuf::traits::Producer;
use crate::audio_input::AudioInput;
use crate::error::Error;

struct AuxiliaryInput {}

impl AuxiliaryInput {
  const CLIENT_NAME: &str = "Input";
  const PORT_NAME: &str = "Auxiliary";
}

impl AudioInput for AuxiliaryInput {
  fn open_stream(
    input_name: &str,
    mut producer: CachingProd<Arc<SharedRb<Heap<f32>>>>
  ) -> Result<Self, Error>
  where
      Self: Sized
  {
    // Create a JACK client and register input port
    let (client, _status) = Client::new(Self::CLIENT_NAME, ClientOptions::default()).unwrap();
    // TODO: Add a unique identifier suffix for PORT_NAME
    let in_port = client.register_port(Self::PORT_NAME, AudioIn::default()).unwrap();
  
    // Create a processing callback that pushes data to ring buffer
    let process = ClosureProcessHandler::new(
      move |_: &Client, ps: &ProcessScope| -> Control {
        let input = in_port.as_slice(ps);
        let _ = producer.push_slice(input);
        Control::Continue
      }
    );
  
    // Activate the client
    let active_client = client.activate_async((), process).unwrap();
  
    // Connect ports to hardware channels
    println!("Connecting ports to system hardware...");
    let client_ptr = active_client.as_client();
  
    let source = input_name;
    let destination= format!("{}:{}", Self::CLIENT_NAME, Self::PORT_NAME);
    if let Err(e) = client_ptr.connect_ports_by_name(&source, &destination) {
      Err(Error {
        message: format!("Could not connect {} to {}: {:?}", source, destination, e),
      })
    } else {
      println!("Connected {} -> {}", source, destination);
      Ok(AuxiliaryInput {})
    }
  }
}
