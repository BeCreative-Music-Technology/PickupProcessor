use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::gain_effect::GainEffect;
use crate::control_input::{ControlInputObserver, RotaryInput};
use crate::audio_effects::low_pass_filter_effect::LowPassFilter;
use crate::routing_director::RoutingDirector;

mod audio_effects;
mod audio_input;
mod auxiliary_input;
mod error;
mod routing_director;
mod audio_bus;
mod audio_output;
mod auxiliary_output;
mod control_input;

const BUFFER_LENGTH: usize = 1024;

fn main() {
    // Create a new routing director
    let mut routing_director = RoutingDirector::new("system:capture_1", BUFFER_LENGTH)
        .expect("Could not initialize routing director");

    // Create and enable a new audio bus
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

    // Create a new control input
    let volume_dial = RotaryInput::new();

    // Add effects to the audio bus
    routing_director.audio_buses().iter_mut().for_each(|bus| {
        // Gain effect
        bus.add_effect(Box::new(GainEffect::new()));
        let mut gain_effect = GainEffect::new();

        let gain_observer = gain_effect.get_control_observer("gain")
            .expect("Could not get observer from effect");

        volume_dial.observable.register(gain_observer);

        bus.add_effect(Box::new(gain_effect));
        bus.for_effect(0, |effect| effect
            .set_value("gain", 32767)
            .expect("Could not set gain value")
        ).expect("Could not add gain effect");

        // Low pass filter effect
        bus.add_effect(Box::new(LowPassFilter::new()));
    });

    loop {
        routing_director.update();
    }
}
