use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use rppal::gpio::{Gpio, InputPin, Level, OutputPin};
use crate::control_input::{ControlChange, ControlInput, ObservableControlInput};
use crate::logger;

///
/// One laser string. All strings share the same ultrasonic sensor, which is read on its own thread
/// started by `LaserStringInput::create_all()`.
/// While a string's beam is broken, the distance measured by the ultrasonic sensor is sent as its value.
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

  fn kind(&self) -> &str {
    "laser"
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
      .map(|index| {
        // Offline until the sensors are initialized, so the panel can tell when they are not working
        let observable = Arc::new(ObservableControlInput::new());
        observable.set_online(false);
        LaserStringInput { observable, laser_id: format!("laser_{}", index) }
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

///
/// HC-SR04 style ultrasonic sensor: a 10 us pulse on TRIG starts a measurement, after which ECHO stays high
/// for as long as the sound takes to travel to the object and back.
///
struct Ultrasonic {
  trigger_pin: OutputPin,
  echo_pin: InputPin,
}

impl Ultrasonic {
  const TRIGGER_PULSE: Duration = Duration::from_micros(10);
  // The echo line goes high a few hundred microseconds after the trigger; give up if it takes much longer
  const ECHO_START_TIMEOUT: Duration = Duration::from_millis(10);
  // Round trip for LaserSensors::VALID_LIMIT (2 m) is ~11.7 ms; a longer echo is treated as no reading
  const ECHO_END_TIMEOUT: Duration = Duration::from_millis(12);
  // Speed of sound in mm/s at ~20 degrees Celsius
  const SPEED_OF_SOUND: f32 = 343_000.0;

  ///
  /// Measures the distance to the nearest object in mm, or `None` when there is no echo.
  ///
  fn measure(&mut self) -> Option<u16> {
    self.trigger_pin.set_high();
    Self::spin(Self::TRIGGER_PULSE);
    self.trigger_pin.set_low();

    let echo_start = self.wait_for(Level::High, Self::ECHO_START_TIMEOUT)?;
    let echo_end = self.wait_for(Level::Low, Self::ECHO_END_TIMEOUT)?;

    Some(Self::echo_to_distance(echo_end.duration_since(echo_start)))
  }

  /// Busy-waits for the echo pin to reach `level`; thread::sleep is far too coarse for microsecond pulses.
  fn wait_for(&self, level: Level, timeout: Duration) -> Option<Instant> {
    let start = Instant::now();
    while self.echo_pin.read() != level {
      if start.elapsed() > timeout {
        return None;
      }
      std::hint::spin_loop();
    }
    Some(Instant::now())
  }

  fn spin(duration: Duration) {
    let start = Instant::now();
    while start.elapsed() < duration {
      std::hint::spin_loop();
    }
  }

  /// The sound travels to the object and back, so the distance is half of what the pulse length suggests.
  fn echo_to_distance(echo: Duration) -> u16 {
    (echo.as_secs_f32() * Self::SPEED_OF_SOUND / 2.0).round().min(u16::MAX as f32) as u16
  }
}

struct LaserSensors {
  // Latest valid reading of the ultrasonic sensor in mm, or NO_READING; written by the sensor thread
  distance: Arc<AtomicU16>,
  photosensor_read_pin: InputPin,
  photosensor_mux_pins: (OutputPin, OutputPin),
}

impl LaserSensors {
  const ULTRASONIC_TRIGGER_PIN: u8 = 17;
  const ULTRASONIC_ECHO_PIN: u8 = 27;
  // HC-SR04 datasheet asks for at least 60 ms between measurements so echoes of the previous one die out
  const ULTRASONIC_INTERVAL: Duration = Duration::from_millis(60);
  const NO_READING: u16 = u16::MAX;
  const MIN_DISTANCE: u16 = 50;
  const MAX_DISTANCE: u16 = 300;
  // Anything beyond this is treated as no reading
  const VALID_LIMIT: u16 = 2000;
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
    logger::info(LOG_ENVIRONMENT, "ultrasonic sensor and photosensors initialized");
    strings.iter().for_each(|(_, observable)| observable.set_online(true));

    let mut beam_broken = vec![false; strings.len()];
    let mut last_values: Vec<Option<u16>> = vec![None; strings.len()];

    loop {
      for (index, (laser_id, observable)) in strings.iter().enumerate() {
        let broken = sensors.read_photosensor(index) == Level::Low;
        if broken != beam_broken[index] {
          beam_broken[index] = broken;
          let distance = match sensors.distance() {
            Some(mm) => format!("{} mm", mm),
            None => "no reading".to_string(),
          };
          logger::log(LOG_ENVIRONMENT, &format!("{} {} (distance: {})", laser_id, if broken { "touched" } else { "released" }, distance));

          // A binding may return to its base value on release, so resend the first reading of every touch
          last_values[index] = None;
          observable.notify_touch(broken);
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
      .map(|pin| pin.into_output_low())
      .map_err(|e| format!("could not claim GPIO {}: {}", pin, e));

    let ultrasonic = Ultrasonic {
      trigger_pin: output_pin(Self::ULTRASONIC_TRIGGER_PIN)?,
      echo_pin: gpio.get(Self::ULTRASONIC_ECHO_PIN)
        .map(|pin| pin.into_input_pulldown())
        .map_err(|e| format!("could not claim GPIO {}: {}", Self::ULTRASONIC_ECHO_PIN, e))?,
    };

    let photosensor_read_pin = gpio.get(Self::PHOTOSENSOR_READ_PIN)
      .map(|pin| pin.into_input_pulldown())
      .map_err(|e| format!("could not claim GPIO {}: {}", Self::PHOTOSENSOR_READ_PIN, e))?;
    let photosensor_mux_pins = (output_pin(Self::PHOTOSENSOR_MUX_S0_PIN)?, output_pin(Self::PHOTOSENSOR_MUX_S1_PIN)?);

    // Start measuring only once every pin was claimed, so a failed init does not leave a thread behind
    let distance = Arc::new(AtomicU16::new(Self::NO_READING));
    Self::spawn_ultrasonic_thread(ultrasonic, distance.clone());

    Ok(Self { distance, photosensor_read_pin, photosensor_mux_pins })
  }

  ///
  /// Measuring blocks for up to a few milliseconds, so it runs on its own thread to keep the
  /// photosensor polling responsive.
  ///
  fn spawn_ultrasonic_thread(mut ultrasonic: Ultrasonic, distance: Arc<AtomicU16>) {
    thread::spawn(move || loop {
      let reading = ultrasonic.measure()
        .filter(|&d| d < Self::VALID_LIMIT)
        .unwrap_or(Self::NO_READING);
      distance.store(reading, Ordering::Relaxed);
      thread::sleep(Self::ULTRASONIC_INTERVAL);
    });
  }

  ///
  /// Latest distance in mm, if the sensor currently sees something.
  ///
  fn distance(&self) -> Option<u16> {
    match self.distance.load(Ordering::Relaxed) {
      Self::NO_READING => None,
      distance => Some(distance),
    }
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
  /// Maps a distance in mm onto the full control range: `MIN_DISTANCE` is 0, `MAX_DISTANCE` is `u16::MAX`.
  ///
  fn to_control_value(distance: u16) -> u16 {
    let clamped = distance.clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);
    let normalized = (clamped - Self::MIN_DISTANCE) as f32 / (Self::MAX_DISTANCE - Self::MIN_DISTANCE) as f32;
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

  #[test]
  fn echo_length_converts_to_distance() {
    // 1 m of travel each way at 343 m/s
    assert_eq!(Ultrasonic::echo_to_distance(Duration::from_micros(5831)), 1000);
    assert_eq!(Ultrasonic::echo_to_distance(Duration::from_micros(0)), 0);
  }
}