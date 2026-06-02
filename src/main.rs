use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::gain_effect::GainEffect;
use crate::audio_effects::low_pass_filter_effect::LowPassFilter;
use crate::error::Error;
use crate::external_connection::Connection;
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
    let mut routing_director = RoutingDirector::new("system:capture_1", BUFFER_LENGTH)
        .expect("Could not initialize routing director");

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

    let mut external_connection: VcsgpConnection = VcsgpConnection::new("")
        .expect("Failed to create VCSGP connection");

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
