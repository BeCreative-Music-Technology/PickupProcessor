use std::sync::Arc;
use std::thread;
use std::time::Duration;
use rppal::gpio::{Gpio, InputPin, Level, OutputPin};
use rppal::hal::Delay;
use rppal::i2c::I2c;
use vl53l0x_simple::Vl53l0x;
use crate::control_input::{ControlChange, ControlInput, ObservableControlInput};
use crate::logger;

///
/// One laser string. All strings share the same sensors, which are read on a single thread
/// started by `LaserStringInput::create_all()`.
/// While a string's beam is broken, the distance measured by the ToF sensors is sent as its value.
///
pub struct LaserStringInput {
  observable: Arc<ObservableControlInput>,
  laser_id: String,
}

static LOG_ENVIRONMENT: &str = "LaserInput";

impl ControlInput for LaserStringInput {
  fn id(&self) -> &str {
    &self.laser_id
  }

  fn observable(&self) -> Arc<ObservableControlInput> {
    self.observable.clone()
  }
}

impl LaserStringInput {
  pub const STRING_COUNT: usize = 3;

  ///
  /// Creates the control inputs `laser_0` .. `laser_2` and starts reading the sensors.
  ///
  pub fn create_all() -> Vec<LaserStringInput> {
    let strings: Vec<LaserStringInput> = (0..Self::STRING_COUNT)
      .map(|index| LaserStringInput {
        observable: Arc::new(ObservableControlInput::new()),
        laser_id: format!("laser_{}", index),
      })
      .collect();

    let observables: Vec<(String, Arc<ObservableControlInput>)> = strings
      .iter()
      .map(|string| (string.laser_id.clone(), string.observable.clone()))
      .collect();
    thread::spawn(move || LaserSensors::run(observables));

    logger::info(LOG_ENVIRONMENT, &format!("created {} laser strings", Self::STRING_COUNT));
    strings
  }
}

struct LaserSensors {
  tof_sensors: [Vl53l0x<I2c, OutputPin>; 2],
  // Latest valid reading of each ToF sensor in mm; the sensors only produce a new one every measurement period
  distances: [Option<u16>; 2],
  photosensor_read_pin: InputPin,
  photosensor_mux_pins: (OutputPin, OutputPin),
}

impl LaserSensors {
  const TOF_X_SHUT_1: u8 = 8;
  const TOF_X_SHUT_2: u8 = 7;
  const TOF_ADDRESSES: [u8; 2] = [0x30, 0x31];
  const TOF_MIN_DISTANCE: u16 = 50;
  const TOF_MAX_DISTANCE: u16 = 300;
  // The VL53L0X reports ~8190 mm when nothing is in range; anything beyond this is treated as no reading
  const TOF_VALID_LIMIT: u16 = 2000;
  const PHOTOSENSOR_READ_PIN: u8 = 10;
  const PHOTOSENSOR_MUX_S0_PIN: u8 = 11;
  const PHOTOSENSOR_MUX_S1_PIN: u8 = 9;
  // (S0, S1) multiplexer selection per string
  const PHOTOSENSOR_SELECTORS: [(bool, bool); LaserStringInput::STRING_COUNT] = [
    (false, false),
    (false, true),
    (true, false),
  ];
  const POLL_INTERVAL: Duration = Duration::from_millis(1);

  fn run(strings: Vec<(String, Arc<ObservableControlInput>)>) {
    let mut sensors = match Self::init() {
      Ok(sensors) => sensors,
      Err(e) => {
        logger::error_str(LOG_ENVIRONMENT, &format!("laser strings disabled: {}", e));
        return;
      }
    };
    logger::info(LOG_ENVIRONMENT, "ToF sensors and photosensors initialized");

    let mut beam_broken = vec![false; strings.len()];
    let mut last_values: Vec<Option<u16>> = vec![None; strings.len()];

    loop {
      sensors.poll_tof_sensors();

      for (index, (laser_id, observable)) in strings.iter().enumerate() {
        let broken = sensors.read_photosensor(index) == Level::Low;
        if broken != beam_broken[index] {
          beam_broken[index] = broken;
          logger::log(LOG_ENVIRONMENT, &format!("{} {}", laser_id, if broken { "touched" } else { "released" }));
        }
        if !broken {
          continue;
        }

        // Only notify when the value changed; the value is held after the string is released
        let Some(value) = sensors.distance().map(Self::to_control_value) else { continue };
        if last_values[index] != Some(value) {
          last_values[index] = Some(value);
          observable.notify(&ControlChange { control_id: laser_id.clone(), value });
        }
      }

      thread::sleep(Self::POLL_INTERVAL);
    }
  }

  fn init() -> Result<Self, String> {
    let gpio = Gpio::new().map_err(|e| format!("could not open GPIO: {}", e))?;
    let output_pin = |pin: u8| gpio.get(pin)
      .map(|pin| pin.into_output())
      .map_err(|e| format!("could not claim GPIO {}: {}", pin, e));

    // Hold both ToF sensors in reset, then bring them up one at a time so each can get its own I2C address
    let mut x_shut_pins = [output_pin(Self::TOF_X_SHUT_1)?, output_pin(Self::TOF_X_SHUT_2)?];
    x_shut_pins.iter_mut().for_each(|pin| pin.set_low());
    thread::sleep(Duration::from_millis(100));

    let mut delay = Delay::new();
    let [x_shut_1, x_shut_2] = x_shut_pins;
    let mut tof_sensor = |x_shut: OutputPin, index: usize| -> Result<Vl53l0x<I2c, OutputPin>, String> {
      let i2c = I2c::new().map_err(|e| format!("could not open I2C (is it enabled in config.txt?): {}", e))?;
      Vl53l0x::new(i2c, x_shut, Self::TOF_ADDRESSES[index], &mut delay)
        .map_err(|e| format!("ToF sensor {} did not initialize: {:?}", index + 1, e))
    };
    let tof_sensors = [tof_sensor(x_shut_1, 0)?, tof_sensor(x_shut_2, 1)?];

    let photosensor_read_pin = gpio.get(Self::PHOTOSENSOR_READ_PIN)
      .map(|pin| pin.into_input_pulldown())
      .map_err(|e| format!("could not claim GPIO {}: {}", Self::PHOTOSENSOR_READ_PIN, e))?;
    let photosensor_mux_pins = (output_pin(Self::PHOTOSENSOR_MUX_S0_PIN)?, output_pin(Self::PHOTOSENSOR_MUX_S1_PIN)?);

    Ok(Self { tof_sensors, distances: [None; 2], photosensor_read_pin, photosensor_mux_pins })
  }

  ///
  /// Stores new ToF readings when the sensors have finished a measurement.
  ///
  fn poll_tof_sensors(&mut self) {
    for (index, tof) in self.tof_sensors.iter_mut().enumerate() {
      match tof.try_read() {
        Ok(Some(distance)) => {
          self.distances[index] = (distance < Self::TOF_VALID_LIMIT).then_some(distance);
        }
        Ok(None) => {} // Still measuring, keep the previous reading
        Err(e) => logger::error_str(LOG_ENVIRONMENT, &format!("ToF sensor {} read error: {:?}", index + 1, e)),
      }
    }
  }

  ///
  /// Average distance of the sensors that currently see something, in mm.
  ///
  fn distance(&self) -> Option<u16> {
    let valid: Vec<u32> = self.distances.iter().flatten().map(|&d| d as u32).collect();
    if valid.is_empty() {
      return None;
    }
    Some((valid.iter().sum::<u32>() / valid.len() as u32) as u16)
  }

  fn read_photosensor(&mut self, index: usize) -> Level {
    let (s0, s1) = Self::PHOTOSENSOR_SELECTORS[index];
    let (s0_pin, s1_pin) = &mut self.photosensor_mux_pins;
    if s0 { s0_pin.set_high() } else { s0_pin.set_low() };
    if s1 { s1_pin.set_high() } else { s1_pin.set_low() };

    // Give the multiplexer time to settle
    thread::sleep(Duration::from_micros(10));

    self.photosensor_read_pin.read()
  }

  ///
  /// Maps a distance in mm onto the full control range: `TOF_MIN_DISTANCE` is 0, `TOF_MAX_DISTANCE` is `u16::MAX`.
  ///
  fn to_control_value(distance: u16) -> u16 {
    let clamped = distance.clamp(Self::TOF_MIN_DISTANCE, Self::TOF_MAX_DISTANCE);
    let normalized = (clamped - Self::TOF_MIN_DISTANCE) as f32 / (Self::TOF_MAX_DISTANCE - Self::TOF_MIN_DISTANCE) as f32;
    (normalized * u16::MAX as f32).round() as u16
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn distance_maps_onto_full_control_range() {
    assert_eq!(LaserSensors::to_control_value(0), 0);
    assert_eq!(LaserSensors::to_control_value(50), 0);
    assert_eq!(LaserSensors::to_control_value(175), 32768);
    assert_eq!(LaserSensors::to_control_value(300), u16::MAX);
    assert_eq!(LaserSensors::to_control_value(8190), u16::MAX);
  }
}
