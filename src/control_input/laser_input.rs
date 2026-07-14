use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use rppal::gpio::{Gpio, InputPin, Level, OutputPin};
use rppal::hal::Delay;
use rppal::i2c::{Error, I2c};
use vl53l0x_simple::{Vl53l0x};
use crate::control_input::{input_helper, ControlChange, ControlInput, ObservableControlInput};
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
  const TOF_X_SHUT_1: u8 = 8;
  const TOF_X_SHUT_2: u8 = 7;
  const TOF_MIN_DISTANCE: u16 = 50;
  const TOF_MAX_DISTANCE: u16 = 300;
  const PHOTOSENSOR_READ_PIN: u8 = 10;
  const PHOTOSENSOR_MUX_S0_PIN: u8 = 11;
  const PHOTOSENSOR_MUX_S1_PIN: u8 = 9;

  fn process(observable: Arc<ObservableControlInput>) {
    let gpio = Gpio::new().expect("Failed to initialize GPIO");

    // Time of flight sensors
    let mut x_shut_1 = gpio.get(Self::TOF_X_SHUT_1).unwrap().into_output();
    let mut x_shut_2 = gpio.get(Self::TOF_X_SHUT_2).unwrap().into_output();
    let mut delay = Delay::new();

    x_shut_1.set_low();
    x_shut_2.set_low();

    input_helper::sleep_millis(100);

    x_shut_1.set_high();
    input_helper::sleep_millis(10);
    let mut tof_1 = match Vl53l0x::new(
      I2c::new().expect("Failed to initialize I2C"),
      x_shut_1,
      0x30,
      &mut delay
    ) {
      Ok(tof) => tof,
      Err(e) => {
        logger::error_str(LOG_ENVIRONMENT, &format!("{:?}", e));
        return
      }
    };

    x_shut_2.set_high();
    input_helper::sleep_millis(10);
    let mut tof_2 = match Vl53l0x::new(
      I2c::new().expect("Failed to initialize I2C"),
      x_shut_2,
      0x31,
      &mut delay
    ) {
      Ok(tof) => tof,
      Err(e) => {
        logger::error_str(LOG_ENVIRONMENT, &format!("{:?}", e));
        return
      }
    };

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
        let readout = Self::read_tof_sensors(&mut tof_1, &mut tof_2);
        observable.notify(&ControlChange {
          control_id: "laser_0".to_string(),
          value: Self::parse_to_cc_value(readout),
        });
      }
      if read_light_sensor(1) == Level::Low {
        let readout = Self::read_tof_sensors(&mut tof_1, &mut tof_2);
        observable.notify(&ControlChange {
          control_id: "laser_1".to_string(),
          value: Self::parse_to_cc_value(readout),
        });
      }
      if read_light_sensor(2) == Level::Low {
        let readout = Self::read_tof_sensors(&mut tof_1, &mut tof_2);
        observable.notify(&ControlChange {
          control_id: "laser_2".to_string(),
          value: Self::parse_to_cc_value(readout),
        });
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

  fn read_tof_sensors(
    tof_1: &mut Vl53l0x<I2c, OutputPin>,
    tof_2: &mut Vl53l0x<I2c, OutputPin>,
  ) -> u16 {
    let res_1 = match tof_1.try_read() {
      Ok(Some(v)) => v,
      Ok(None) => return 0,
      Err(e) => {
        logger::error_str(LOG_ENVIRONMENT, &format!("TOF2 read error: {:?}", e));
        return 0;
      }
    };
    let res_2 = match tof_2.try_read() {
      Ok(Some(v)) => v,
      Ok(None) => return 0,
      Err(e) => {
        logger::error_str(LOG_ENVIRONMENT, &format!("TOF2 read error: {:?}", e));
        return 0;
      }
    };

    res_1 + res_2 / 2
  }

  fn parse_to_cc_value(value: u16) -> u16 {
    let normalized = (value - Self::TOF_MIN_DISTANCE) / (Self::TOF_MAX_DISTANCE - Self::TOF_MIN_DISTANCE);
    normalized * (u16::MAX - u16::MIN) + u16::MIN
  }
}
