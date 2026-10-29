use std::env;
use std::process;
use std::sync::{Arc, Mutex};
use std::thread;
use crate::routing_director::RoutingDirector;
use crate::control_input::{ControlInput, LaserStringInput, RotaryInput};
use crate::external_connection::{ExternalConnection, VcsgpConnection};

mod audio_effects;
mod audio_input;
mod auxiliary_input;
mod error;
mod routing_director;
mod audio_bus;
mod audio_output;
mod auxiliary_output;
mod control_input;
mod control_feedback;
mod logger;
pub mod external_connection;

static LOG_ENVIRONMENT: &str = "Main";

fn main() {
    let buffer_length = match env::args().nth(1).map(|arg| arg.parse::<usize>()) {
        None => 512,
        Some(Ok(buffer_length)) if buffer_length > 0 => buffer_length,
        Some(_) => {
            logger::error_str(LOG_ENVIRONMENT, "buffer length must be a positive whole number, e.g. `pickup-processor 512`");
            process::exit(1);
        }
    };
    logger::info(LOG_ENVIRONMENT, &format!("using buffer length of {}", buffer_length));

    // Create a new routing director
    let mut routing_director = RoutingDirector::new("system:capture_4", buffer_length)
        .expect("Could not initialize routing director");

    // Instantiate audio buses
    ["system:playback_4", "system:playback_5", "system:playback_6"]
        .iter().for_each(|output_id| {
        routing_director
            .add_audio_bus(output_id)
            .expect("Could not instantiate new audio bus");
        });

    let routing_director = Arc::new(Mutex::new(routing_director));

    let control_inputs: Arc<Mutex<Vec<Box<dyn ControlInput>>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let mut control_inputs = control_inputs.lock().unwrap();
        control_inputs.push(Box::new(RotaryInput::new()));
        for laser_string in LaserStringInput::create_all() {
            control_inputs.push(Box::new(laser_string));
        }
    }

    // Setup VCSGP connection
    let protocol_connection = VcsgpConnection::new("[::]:31628")
        .expect("Failed to create VCSGP connection");
    protocol_connection.start(Arc::clone(&routing_director), control_inputs);

    loop {
        let routed_samples = routing_director.lock().unwrap().update();

        // Nothing captured yet; back off instead of spinning on the lock
        if routed_samples == 0 {
            thread::sleep(audio_bus::IDLE_SLEEP);
        }
    }
}
