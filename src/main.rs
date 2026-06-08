use std::sync::{Arc, Mutex};
use portal::external_connection::ExternalConnection;
use crate::routing_director::RoutingDirector;
use portal::vcsgp_connection::VcsgpConnection;

mod audio_effects;
mod audio_input;
mod auxiliary_input;
mod error;
mod routing_director;
mod audio_bus;
mod audio_output;
mod auxiliary_output;
mod control_input;
mod portal;

const BUFFER_LENGTH: usize = 1024;

fn main() {
    // Create a new routing director
    let routing_director_pointer = Arc::new(Mutex::new(RoutingDirector::new("system:capture_1", BUFFER_LENGTH)
        .expect("Could not initialize routing director")));
    let routing_director_clone = routing_director_pointer.clone();
    let mut routing_director = routing_director_pointer.lock().unwrap();

    // Instantiate audio buses
    ["system:playback_1", "system:playback_2", "system:playback_3", "system:playback_4"]
        .iter().for_each(|output_id| {
        routing_director
            .add_audio_bus(output_id)
            .expect("Could not instantiate new audio bus");
        });

    // Enable audio buses
    let bus_ids: Vec<_> = routing_director
        .audio_buses()
        .iter()
        .map(|bus| bus.id().to_string())
        .collect();
    for id in bus_ids {
        routing_director
            .enable_audio_bus(&id)
            .expect("Audio bus could not be enabled");
    }
    // TODO: Check if mutex lock is dropped

    let control_inputs = Arc::new(Mutex::new(Vec::new()));

    let mut protocol_connection = VcsgpConnection::new("127.0.0.1:31628")
        .expect("Failed to create VCSGP connection");
    protocol_connection.start(routing_director_clone, control_inputs);

    loop {
        routing_director.update();
    }
}
