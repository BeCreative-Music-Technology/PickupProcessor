use std::sync::atomic::{AtomicU8, Ordering};
use jack::{AudioIn, Client, ClientOptions, Control, ProcessScope};
use jack::contrib::ClosureProcessHandler;
use rtrb::Producer;
use crate::audio_input::AudioInput;
use crate::error::Error;
use crate::logger;

pub struct AuxiliaryInput {
  // Keeps the JACK client active; dropping it deactivates the client.
  client: Option<Box<dyn Send + Sync>>,
  aux_id: String,
  sample_rate: usize,
}

impl AuxiliaryInput {
  const CLIENT_NAME: &str = "Input";
  const PORT_NAME: &str = "Auxiliary";
}

static AUX_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);
static LOG_ENVIRONMENT: &str = "AuxiliaryInput";

impl AudioInput for AuxiliaryInput
{
  ///
  /// Opens an `AuxiliaryInput` stream using Jack.
  /// The JACK client stays active until `close_stream()` is called or the input is dropped.
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
    let incremental_id = AUX_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed);
    let client_name = format!("{}_{}", Self::CLIENT_NAME, incremental_id);
    let (client, _status) = Client::new(&client_name, ClientOptions::default())
      .map_err(|e| Error::new(&format!("could not create JACK client {}: {:?}", client_name, e)))?;
    let port_name = format!("{}_{}", Self::PORT_NAME, incremental_id);
    let in_port = client.register_port(&port_name, AudioIn::default())
      .map_err(|e| Error::new(&format!("could not register JACK port {}: {:?}", port_name, e)))?;
    let aux_id = format!("{}:{}", client_name, port_name);
    let sample_rate = client.sample_rate() as usize;

    // Create a processing callback that pushes data to ring buffer.
    // Writes as much as fits so a briefly full buffer drops samples instead of the whole period.
    let process = ClosureProcessHandler::new(
      move |_: &Client, ps: &ProcessScope| -> Control {
        let input = in_port.as_slice(ps);
        let writable = producer.slots().min(input.len());
        if let Ok(chunk) = producer.write_chunk_uninit(writable) {
          chunk.fill_from_iter(input.iter().copied());
        }
        Control::Continue
      }
    );

    // Activate client and connect ports to hardware channels
    let active_client = client.activate_async((), process)
      .map_err(|e| Error::new(&format!("could not activate JACK client {}: {:?}", client_name, e)))?;

    if let Err(e) = active_client.as_client().connect_ports_by_name(input_name, &aux_id) {
      logger::error_str(LOG_ENVIRONMENT, &format!("Could not connect {} to {}: {:?}", input_name, aux_id, e));
    } else {
      logger::info(LOG_ENVIRONMENT, &format!("Connected {} -> {}", input_name, aux_id));
    }

    Ok(AuxiliaryInput {
      client: Some(Box::new(active_client)),
      aux_id,
      sample_rate,
    })
  }

  ///
  /// Closes the `AuxiliaryInput` stream by deactivating the JACK client.
  ///
  fn close_stream(&mut self) {
    self.client.take();
  }

  ///
  /// Returns the incremental id of the `AuxiliaryInput`.
  ///
  fn id(&self) -> &str {
    self.aux_id.as_str()
  }

  ///
  /// Returns the sample rate of the JACK server in Hz.
  ///
  fn sample_rate(&self) -> usize {
    self.sample_rate
  }
}
