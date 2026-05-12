use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::thread::JoinHandle;
use jack::{AudioOut, Client, ClientOptions, Control, ProcessScope};
use jack::contrib::ClosureProcessHandler;
use ringbuf::{CachingCons, SharedRb};
use ringbuf::storage::Heap;
use ringbuf::traits::Consumer;
use crate::audio_output::AudioOutput;
use crate::error::Error;

pub struct AuxiliaryOutput {
  thread: Option<JoinHandle<()>>,
  aux_id: String,
}

impl AuxiliaryOutput {
  const CLIENT_NAME: &str = "Output";
  const PORT_NAME: &str = "Auxiliary";
}

static AUX_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);

impl AudioOutput for AuxiliaryOutput {
  fn open_stream(
    output_name: &str,
    mut consumer: CachingCons<Arc<SharedRb<Heap<f32>>>>
  ) -> Result<Self, Error>
  where
      Self: Sized
  {
    // Create a JACK client and register output port
    let (client, _status) = Client::new(Self::CLIENT_NAME, ClientOptions::default()).unwrap();
    let incremental_id = AUX_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed);
    let port_name = format!("{}_{}", Self::PORT_NAME, incremental_id);
    let mut out_port = client.register_port(&port_name, AudioOut::default()).unwrap();
    let aux_id = format!("{}:{}", Self::CLIENT_NAME, port_name);

    // Create a processing callback that reads data from ring buffer
    let process = ClosureProcessHandler::new(
      move |_: &Client, ps: &ProcessScope| -> Control {
        out_port.as_mut_slice(ps).iter_mut().for_each(|mut out_sample| {
          let in_sample  = consumer.try_pop().unwrap_or(0.0);
          *out_sample = in_sample;
        });
        Control::Continue
      }
    );

    // Activate client and connect ports to hardware channels
    let source = output_name.to_owned();
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

    Ok(AuxiliaryOutput {
      thread: Some(handle),
      aux_id,
    })
  }

  ///
  /// Closes the `AuxiliaryOutput` stream by stopping the `std::thread`.
  ///
  fn close_stream(&mut self) {
    if let Some(thread) = self.thread.take() {
      thread.thread().unpark();
      thread.join().unwrap();
    }
  }

  fn id(&self) -> &str {
    self.aux_id.as_str()
  }
}
