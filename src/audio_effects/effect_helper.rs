pub fn map(value: u16, range_min: u16, range_max: u16, value_min: f32, value_max: f32) -> f32 {
  let value_f32 = value as f32;
  let range_min_f32 = range_min as f32;
  let range_max_f32 = range_max as f32;
  let normalized = (value_f32 - range_min_f32) / (range_max_f32 - range_min_f32);
  normalized * (value_max - value_min) + value_min
}

pub fn mix(previous_value: f32, new_value: f32, mix_percentage: f32) -> f32 {
  previous_value * (1.0 - mix_percentage) + new_value * mix_percentage
}

/// Time a smoothed parameter takes to (mostly) reach a new value, short enough to feel immediate.
pub const SMOOTHING_SECONDS: f32 = 0.02;

///
/// One-pole smoothing for parameters that are changed while audio plays (e.g. by a laser string),
/// so a jump in value does not produce an audible click or stepping.
///
pub struct Smoothed {
  current: f32,
  coefficient: f32,
}

impl Smoothed {
  pub fn new(initial: f32, sample_rate: f64) -> Self {
    let coefficient = 1.0 - (-1.0 / (SMOOTHING_SECONDS * sample_rate as f32)).exp();
    Self { current: initial, coefficient }
  }

  /// Moves one sample towards `target` and returns the smoothed value.
  pub fn next(&mut self, target: f32) -> f32 {
    self.current += (target - self.current) * self.coefficient;
    self.current
  }

  #[cfg(test)]
  pub fn value(&self) -> f32 {
    self.current
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn smoothed_value_glides_to_target() {
    let sample_rate = 48000.0;
    let mut smoothed = Smoothed::new(0.0, sample_rate);
    let first = smoothed.next(1.0);
    assert!(first > 0.0 && first < 0.01, "jumped to {}", first);

    // After five time constants the value is within 1% of the target
    let samples = (5.0 * SMOOTHING_SECONDS * sample_rate as f32) as usize;
    (0..samples).for_each(|_| { smoothed.next(1.0); });
    assert!((smoothed.value() - 1.0).abs() < 0.01);
  }
}
