use crate::routing_director::RoutingDirector;

mod audio_input;
mod auxiliary_input;
mod error;
mod routing_director;
mod audio_bus;
mod audio_output;
mod auxiliary_output;

fn main() {
    let mut routing_director = RoutingDirector::new("system:capture_1")
        .expect("Could not initialize routing director");
    
    routing_director
        .add_audio_bus("system:playback_1")
        .expect("Could not instantiate new audio bus");
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

    loop {
        routing_director.update();
    }
}
