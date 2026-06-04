use std::sync::{Arc, Mutex};
use crate::external_connection::ExternalConnection;
use crate::routing_director::RoutingDirector;
use crate::vcsgp_connection::VcsgpConnection;

mod audio_effects;
mod audio_input;
mod auxiliary_input;
mod error;
mod routing_director;
mod audio_bus;
mod audio_output;
mod auxiliary_output;
mod external_connection;
mod vcsgp_connection;

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

    let mut protocol_connection = VcsgpConnection::new("")
        .expect("Failed to create VCSGP connection");
    protocol_connection.start(routing_director_clone);

    // // Add effects to the audio bus
    // routing_director.audio_buses().iter_mut().for_each(|bus| {
    //     // Gain effect
    //     bus.add_effect(Box::new(GainEffect::new()));
    //     bus.for_effect(0, |effect| effect
    //         .set_value("gain", 32767)
    //         .expect("Could not set gain value")
    //     ).expect("Could not add gain effect");
    //
    //     // Low pass filter effect
    //     bus.add_effect(Box::new(LowPassFilter::new()));
    // });

    loop {
        routing_director.update();
    }
}
