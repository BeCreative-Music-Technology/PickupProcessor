use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::thread::JoinHandle;
use jack::{AudioIn, Client, ClientOptions, Control, ProcessScope};
use jack::contrib::ClosureProcessHandler;
use rtrb::Producer;
use crate::audio_input::AudioInput;
use crate::error::Error;

pub struct AuxiliaryInput {
  thread: Option<JoinHandle<()>>,
  aux_id: String,
}

impl AuxiliaryInput {
  const CLIENT_NAME: &str = "Input";
  const PORT_NAME: &str = "Auxiliary";
}

static AUX_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);

impl AudioInput for AuxiliaryInput
{
  ///
  /// Opens an `AuxiliaryInput` stream using Jack.
  /// The stream is kept open using an `std::thread`.
  ///
  /// `input_name` takes an `&str` with the id of the systems auxiliary port connected to the jack server.
  /// An example of this string could be `"system:capture_1"`, which connects to the system's capture 1 port.
  ///
  /// `producer` takes a reference to a mutable ring buffer producer.
  /// The stream will be sent to the ring buffer and can be read using a ring buffer consumer.
  ///
  fn open_stream(
    input_name: &str,
    mut producer: Producer<f32>,
  ) -> Result<Self, Error>
  where
      Self: Sized
  {
    // Create a JACK client and register input port
    let (client, _status) = Client::new(Self::CLIENT_NAME, ClientOptions::default()).unwrap();
    let incremental_id = AUX_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed);
    let port_name = format!("{}_{}", Self::PORT_NAME, incremental_id);
    let in_port = client.register_port(&port_name, AudioIn::default()).unwrap();
    let aux_id = format!("{}:{}", Self::CLIENT_NAME, port_name);

    // Create a processing callback that pushes data to ring buffer
    let process = ClosureProcessHandler::new(
      move |_: &Client, ps: &ProcessScope| -> Control {
        let input = in_port.as_slice(ps);
        let _ = producer.push_entire_slice(input);
        Control::Continue
      }
    );

    // Activate client and connect ports to hardware channels
    let source = input_name.to_owned();
    let destination= aux_id.to_owned();
    let handle = thread::spawn(move || {
      let active_client = client.activate_async((), process).unwrap();

      if let Err(e) = active_client.as_client().connect_ports_by_name(&source, &destination) {
        println!("Could not connect {} to {}: {:?}", source, destination, e);
      } else {
        println!("Connected {} -> {}", source, destination)
      }

      thread::park();
    });

    Ok(AuxiliaryInput {
      thread: Some(handle),
      aux_id,
    })
  }

  ///
  /// Closes the `AuxiliaryInput` stream by stopping the `std::thread`.
  ///
  fn close_stream(&mut self) {
    if let Some(thread) = self.thread.take() {
      thread.thread().unpark();
      thread.join().unwrap();
    }
  }

  ///
  /// Returns the incremental id of the `AuxiliaryInput`.
  ///
  fn id(&self) -> &str {
    self.aux_id.as_str()
  }
}
