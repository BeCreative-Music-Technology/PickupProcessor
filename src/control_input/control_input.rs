use std::thread;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use rppal::gpio::{Gpio, Level};
use super::control_input_observer::ControlChange;
use super::observable_control_input::ObservableControlInput;

pub trait ControlInput: Send + Sync {
    fn new() -> Self where Self: Sized;
    fn id(&self) -> String;
    fn observable(&self) -> Arc<ObservableControlInput>;
}

pub struct RotaryInput {
    observable: Arc<ObservableControlInput>,
    rotary_id: String,
}

static ROTARY_INPUT_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);

impl ControlInput for RotaryInput {
    fn new() -> Self where Self: Sized {
        let rotary_id = format!("rotary_{}", ROTARY_INPUT_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));

        let observable = Arc::new(ObservableControlInput::new());
        let observable_clone = Arc::clone(&observable);

        let gpio = Gpio::new().expect("Failed to initialize GPIO");

        let clk_pin = gpio.get(Self::GPIO_CLK).expect("REASON").into_input_pullup();
        let dt_pin = gpio.get(Self::GPIO_DT).expect("REASON").into_input_pullup();

        thread::spawn(move || {
            let mut encoder_value: u16 = u16::MAX / 2;
            let mut last_clk_level = clk_pin.read();
            loop {
                let current_clk_level = clk_pin.read();
                if current_clk_level != last_clk_level {
                    let current_dt_level = dt_pin.read();
                    if current_clk_level == Level::Low {
                        if current_dt_level == Level::High {
                            encoder_value = encoder_value.saturating_add(256); // Adjust step size to taste
                        } else {
                            encoder_value = encoder_value.saturating_sub(256);
                        }
                        let change = ControlChange {
                            control_id: "encoder_1".to_string(),
                            value: encoder_value
                        };
                        observable_clone.notify(&change);
                    }
                }
                last_clk_level = current_clk_level;
                thread::sleep(Duration::from_micros(500));
            }
        });

        Self { observable, rotary_id }
    }

    fn id(&self) -> String {
        self.rotary_id
    }

    fn observable(&self) -> Arc<ObservableControlInput> {
        self.observable.clone()
    }
}

impl RotaryInput {
    // Define the BCM GPIO pin numbers
    const GPIO_CLK: u8 = 14;
    const GPIO_DT: u8 = 15;
}
