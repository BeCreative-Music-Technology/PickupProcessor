use std::thread;
use std::time::Duration;
use std::sync::Arc;
use rppal::gpio::{Gpio, Level};
use super::control_input_observer::ControlChange;
use super::observable_control_input::ObservableControlInput;


pub trait ControlInput: Send + Sync {}


pub struct RotaryInput {
    pub observable: Arc<ObservableControlInput>,
}

impl ControlInput for RotaryInput {}



impl RotaryInput {
    // Define the BCM GPIO pin numbers
    const GPIO_CLK: u8 = 14;
    const GPIO_DT: u8 = 15;

    pub fn new() -> Self {
        let observable = Arc::new(ObservableControlInput::new());
        let observable_clone = Arc::clone(&observable);

        // Initialize Raspberry Pi GPIO
        let gpio = Gpio::new().expect("Failed to initialize GPIO");

        // Configure CLK and DT pins as inputs with internal pull-up resistors
        let clk_pin = gpio.get(Self::GPIO_CLK).expect("REASON").into_input_pullup();
        let dt_pin = gpio.get(Self::GPIO_DT).expect("REASON").into_input_pullup();

        thread::spawn(move || {
            // Keep track of the value (clamped between u16::MIN and u16::MAX)
            let mut encoder_value: u16 = u16::MAX / 2;

            // Read initial state of the clock pin
            let mut last_clk_level = clk_pin.read();

            loop {
                let current_clk_level = clk_pin.read();

                // Look for a state change on the CLK pin (a pulse edge)
                if current_clk_level != last_clk_level {

                    // If CLK has changed, check the state of the DT pin to figure out direction
                    let current_dt_level = dt_pin.read();

                    if current_clk_level == Level::Low {
                        // Falling edge logic
                        if current_dt_level == Level::High {
                            // Clockwise rotation
                            encoder_value = encoder_value.saturating_add(256); // Adjust step size to taste
                        } else {
                            // Counter-clockwise rotation
                            encoder_value = encoder_value.saturating_sub(256);
                        }

                        // Send the updated value out to all passive audio effect observers
                        let change = ControlChange {
                            control_id: "encoder_1".to_string(),
                            value: encoder_value
                        };
                        observable_clone.notify(&change);
                    }
                }

                // Update the state history
                last_clk_level = current_clk_level;

                // Micro-sleep to avoid pinning the CPU core while maintaining highly responsive sampling
                thread::sleep(Duration::from_micros(500));
            }
        });

        Self { observable }
    }
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