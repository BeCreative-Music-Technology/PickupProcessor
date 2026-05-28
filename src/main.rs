use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::gain_effect::GainEffect;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use crate::control_input::{ControlChange, ControlInputObserver, PotentiometerInput};
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

/// bridge between the observer system and main loop
struct MainValueBridge {
    pub value_storage: Arc<AtomicU16>,
}

impl ControlInputObserver for MainValueBridge {
    fn update(&self, cc: &ControlChange) {
        // When the sensor fires, seamlessly store the new value
        self.value_storage.store(cc.value, Ordering::SeqCst);
    }
}


fn main() {
    let mut routing_director = RoutingDirector::new("system:capture_1", BUFFER_LENGTH)
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

    routing_director.audio_buses().iter_mut().for_each(|bus| {
        bus.add_effect(Box::new(GainEffect::new()));
        bus.for_effect(0, |effect| effect
            .set_value("gain", 32767)
            .expect("Could not set gain value")
        ).expect("Could not add gain effect");
    });

    // create pointer for sensor value
    let shared_sensor_value = Arc::new(AtomicU16::new(0));

    // initialize hardware sensor
    let volume_dial = PotentiometerInput::new();

    // connect the sensor to bridge observer
    let bridge = Arc::new(MainValueBridge {
        value_storage: shared_sensor_value.clone(),
    });
    volume_dial.observable.register(bridge);

    loop {
        routing_director.update();

        // Load the real-time sensor value into main
        let current_value = shared_sensor_value.load(Ordering::SeqCst);

        // Print it out to the console
        println!("Value in main loop: {}", current_value);

        // Optional: Avoid pinning your CPU core to 100% in an empty loop
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}