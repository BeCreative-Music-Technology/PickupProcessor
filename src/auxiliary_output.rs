use std::sync::atomic::{AtomicU8, Ordering};
use jack::{AudioOut, Client, ClientOptions, Control, ProcessScope};
use jack::contrib::ClosureProcessHandler;
use rtrb::Consumer;
use crate::audio_output::AudioOutput;
use crate::error::Error;
use crate::logger;

pub struct AuxiliaryOutput {
  // Keeps the JACK client active; dropping it deactivates the client.
  client: Option<Box<dyn Send + Sync>>,
  aux_id: String,
}

impl AuxiliaryOutput {
  const CLIENT_NAME: &str = "Output";
  const PORT_NAME: &str = "Auxiliary";
}

static AUX_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);
static LOG_ENVIRONMENT: &str = "AuxiliaryOutput";

impl AudioOutput for AuxiliaryOutput {
  ///
  /// Opens an `AuxiliaryOutput` stream using Jack.
  /// The JACK client stays active until `close_stream()` is called or the output is dropped.
  ///
  /// `output_name` takes an `&str` with the id of the systems auxiliary port connected to the jack server.
  /// An example of this string could be `"system:playback_1"`, which connects to the system's playback 1 port.
  ///
  /// `consumer` takes a reference to a mutable ring buffer consumer.
  /// The stream will be read from the ring buffer using it.
  ///
  fn open_stream(
    output_name: &str,
    mut consumer: Consumer<f32>
  ) -> Result<Self, Error>
  where
      Self: Sized
  {
    // Create a JACK client and register output port
    let incremental_id = AUX_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed);
    let client_name = format!("{}_{}", Self::CLIENT_NAME, incremental_id);
    let (client, _status) = Client::new(&client_name, ClientOptions::default())
      .map_err(|e| Error::new(&format!("could not create JACK client {}: {:?}", client_name, e)))?;
    let port_name = format!("{}_{}", Self::PORT_NAME, incremental_id);
    let mut out_port = client.register_port(&port_name, AudioOut::default())
      .map_err(|e| Error::new(&format!("could not register JACK port {}: {:?}", port_name, e)))?;
    let aux_id = format!("{}:{}", client_name, port_name);

    // Create a processing callback that reads data from ring buffer, padding with silence on underrun
    let process = ClosureProcessHandler::new(
      move |_: &Client, ps: &ProcessScope| -> Control {
        let output = out_port.as_mut_slice(ps);
        let readable = consumer.slots().min(output.len());
        let (filled, silent) = output.split_at_mut(readable);
        if let Ok(chunk) = consumer.read_chunk(readable) {
          let (first, second) = chunk.as_slices();
          filled[..first.len()].copy_from_slice(first);
          filled[first.len()..].copy_from_slice(second);
          chunk.commit_all();
        }
        silent.fill(0.0);
        Control::Continue
      }
    );

    // Activate client and connect ports to hardware channels
    let active_client = client.activate_async((), process)
      .map_err(|e| Error::new(&format!("could not activate JACK client {}: {:?}", client_name, e)))?;

    if let Err(e) = active_client.as_client().connect_ports_by_name(&aux_id, output_name) {
      logger::error_str(LOG_ENVIRONMENT, &format!("Could not connect {} to {}: {:?}", aux_id, output_name, e))
    } else {
      logger::info(LOG_ENVIRONMENT, &format!("Connected {} -> {}", aux_id, output_name));
    }

    Ok(AuxiliaryOutput {
      client: Some(Box::new(active_client)),
      aux_id,
    })
  }

  ///
  /// Closes the `AuxiliaryOutput` stream by deactivating the JACK client.
  ///
  fn close_stream(&mut self) {
    self.client.take();
  }

  fn id(&self) -> &str {
    self.aux_id.as_str()
  }
}
