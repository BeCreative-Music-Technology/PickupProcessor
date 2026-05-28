use std::thread;
use std::time::Duration;
use std::sync::Arc;
use super::control_input_observer::ControlChange;
use super::observable_control_input::ObservableControlInput;


pub trait ControlInput: Send + Sync {}


pub struct PotentiometerInput {
    pub observable: Arc<ObservableControlInput>,
}

impl ControlInput for PotentiometerInput {}

impl PotentiometerInput {
    pub fn new() -> Self {
        let observable = ObservableControlInput::new();

        // 1. Wrap the observable or extract a way to notify.
        // Since ObservableControlInput handles its own interior mutability (Mutex),
        // we can wrap it in an Arc to share it safely with the background thread.
        let observable = Arc::new(observable);
        let observable_clone = Arc::clone(&observable);

        thread::spawn(move || {
            let mut test_counter: u16 = 0;
            loop {
                let change = ControlChange {
                    control_id: "pot_1".to_string(),
                    value: test_counter,
                    enabled: true,
                };
                observable_clone.notify(&change);

                test_counter = (test_counter + 1) % 1024;
            }
        });
        
        Self { observable }
    }
}
fn read_physical_potentiometer_pin() -> u16 {
    // e.g., i2c or SPI read code here
    512
}


pub struct LaserInput {
    pub observable: ObservableControlInput,
}

impl LaserInput {
    pub fn new() -> Self {
        Self {
            observable: ObservableControlInput::new(),
        }
    }
}

impl ControlInput for LaserInput {}