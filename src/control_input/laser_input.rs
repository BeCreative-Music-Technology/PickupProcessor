use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use rppal::gpio::{Gpio, InputPin, Level, OutputPin};
use crate::control_input::{input_helper, ControlInput, ObservableControlInput};
use crate::logger;

struct LaserInput {
  observable: Arc<ObservableControlInput>,
  laser_id: String,
  
}

static LASER_INPUT_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);
static LOG_ENVIRONMENT: &str = "LaserInput";

impl ControlInput for LaserInput {
  fn new() -> Self
  where
    Self: Sized
  {
    let laser_id = format!("laser_{}", LASER_INPUT_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));

    let observable = Arc::new(ObservableControlInput::new());
    let observable_clone = Arc::clone(&observable);

    thread::spawn(move || Self::process(observable_clone));

    logger::info(LOG_ENVIRONMENT, "created");

    Self { observable, laser_id }
  }

  fn id(&self) -> &str {
    &self.laser_id
  }

  fn observable(&self) -> Arc<ObservableControlInput> {
    self.observable.clone()
  }
}

impl LaserInput {
  const LASER_DIRECTION_PIN: u8 = 16;
  const LASER_0_INC_PIN: u8 = 5;
  const LASER_1_INC_PIN: u8 = 6;
  const LASER_2_INC_PIN: u8 = 13;
  const PHOTOSENSOR_READ_PIN: u8 = 10;
  const PHOTOSENSOR_MUX_S0_PIN: u8 = 11;
  const PHOTOSENSOR_MUX_S1_PIN: u8 = 9;

  fn process(observable: Arc<ObservableControlInput>) {
    let gpio = Gpio::new().expect("Failed to initialize GPIO");

    // Laser module
    let mut laser_direction_pin = gpio.get(Self::LASER_DIRECTION_PIN).unwrap().into_output();
    let mut laser_0_inc_pin = gpio.get(Self::LASER_0_INC_PIN).unwrap().into_output();
    laser_0_inc_pin.set_high();
    let mut laser_1_inc_pin = gpio.get(Self::LASER_1_INC_PIN).unwrap().into_output();
    laser_1_inc_pin.set_high();
    let mut laser_2_inc_pin = gpio.get(Self::LASER_2_INC_PIN).unwrap().into_output();
    laser_2_inc_pin.set_high();

    let set_laser_0_brightness = Self::set_laser_brightness_factory(&mut laser_direction_pin, laser_0_inc_pin);
    let set_laser_1_brightness = Self::set_laser_brightness_factory(&mut laser_direction_pin, laser_1_inc_pin);
    let set_laser_2_brightness = Self::set_laser_brightness_factory(&mut laser_direction_pin, laser_2_inc_pin);

    // Light sensor module
    let photosensor_read_pin = gpio.get(Self::PHOTOSENSOR_READ_PIN).unwrap().into_input_pulldown();
    let photosensor_s0_mux_pin = gpio.get(Self::PHOTOSENSOR_MUX_S0_PIN).unwrap().into_output();
    let photosensor_s1_mux_pin = gpio.get(Self::PHOTOSENSOR_MUX_S1_PIN).unwrap().into_output();

    let read_light_sensor = Self::read_light_sensor_factory(
      photosensor_read_pin,
      photosensor_s0_mux_pin,
      photosensor_s1_mux_pin
    );

    loop {
      if read_light_sensor(0) == Level::Low {
        set_laser_0_brightness(90);
      } else {
        set_laser_0_brightness(100);
      }
      if read_light_sensor(1) == Level::Low {
        set_laser_1_brightness(90);
      } else {
        set_laser_1_brightness(100);
      }
      if read_light_sensor(2) == Level::Low {
        set_laser_2_brightness(90);
      } else {
        set_laser_2_brightness(100);
      }
    }
  }

  fn read_light_sensor_factory(
    read_pin: InputPin,
    mut s0_mux_pin: OutputPin,
    mut s1_mux_pin: OutputPin
  ) -> Box<dyn FnMut(u8) -> Level>
  {
    let selector_maps: [(bool, bool); 3] = [
      (false, false),
      (false, true),
      (true, false),
    ];

    Box::new(move |index| -> Level {
      let selector_map = selector_maps[index as usize];
      if selector_map.0 { s0_mux_pin.set_high() } else { s0_mux_pin.set_low() };
      if selector_map.1 { s1_mux_pin.set_high() } else { s1_mux_pin.set_low() };

      input_helper::sleep_micros(10);

      read_pin.read()
    })
  }

  fn set_laser_brightness_factory(direction_pin: &mut OutputPin, mut increment_pin: OutputPin) -> Box<dyn FnMut(u8)> {
    let mut step_count = 100_u8;

    // Reset laser potentiometer to zero resistance (full brightness)
    for _ in 101 {
      Self::set_wiper_direction(direction_pin, Direction::Down);

      increment_pin.set_low();
      input_helper::sleep_micros(5);
      increment_pin.set_high();
      input_helper::sleep_micros(5);
    }

    Box::new(move |brightness| {
      while step_count != brightness {
        if step_count < brightness {
          step_count += 1;
          Self::set_wiper_direction(direction_pin, Direction::Up);

          increment_pin.set_low();
          input_helper::sleep_micros(5);
          increment_pin.set_high();
          input_helper::sleep_micros(5);
        }
        else {
          step_count -= 1;
          Self::set_wiper_direction(direction_pin, Direction::Down);

          increment_pin.set_low();
          input_helper::sleep_micros(5);
          increment_pin.set_high();
          input_helper::sleep_micros(5);
        }
      }
    })
  }

  fn set_wiper_direction(direction_pin: &mut OutputPin, direction: Direction) {
    if direction_pin.is_set_high() && direction == Direction::Down {
      direction_pin.set_low();
      input_helper::sleep_micros(5);
    }
    else if direction_pin.is_set_low() && direction == Direction::Up {
      direction_pin.set_high();
      input_helper::sleep_micros(5);
    }
  }
}


#[derive(Eq, PartialEq)]
enum Direction {
  Up,
  Down,
}
