use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use serde::{Deserialize, Serialize};
use crate::control_input::{ControlChange, ControlInputObserver};

/// How the position of a control is shaped before it is mapped onto the parameter range.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
  #[default]
  Linear,
  /// Slow at the start, fast at the end; feels natural for frequencies.
  Exponential,
  /// Fast at the start, slow at the end.
  Logarithmic,
}

/// What a parameter does when a touchable control (a laser string) is released.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Release {
  /// Keep the last value.
  #[default]
  Hold,
  /// Go back to the value the parameter was set to in the panel.
  Return,
}

/// How a touchable control switches an effect on and off.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GateMode {
  /// The effect is only on while the control is touched.
  #[default]
  Momentary,
  /// Every touch switches the effect on or off.
  Toggle,
}

///
/// Maps a control position (`0..=u16::MAX`) onto part of a parameter's range.
/// `range_min` may be larger than `range_max` to invert the direction.
///
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMapping {
  pub range_min: u16,
  pub range_max: u16,
  pub curve: Curve,
  pub release: Release,
  /// Value the parameter returns to with `Release::Return`.
  pub base: u16,
}

impl ControlMapping {
  // Steepness of the exponential and logarithmic curves
  const CURVE_STEEPNESS: f32 = 5.0;

  #[cfg(test)]
  pub fn full_range(base: u16) -> Self {
    Self { range_min: 0, range_max: u16::MAX, curve: Curve::Linear, release: Release::Hold, base }
  }

  pub fn map(&self, control_value: u16) -> u16 {
    let position = control_value as f32 / u16::MAX as f32;
    let scale = 2.0_f32.powf(Self::CURVE_STEEPNESS) - 1.0;
    let shaped = match self.curve {
      Curve::Linear => position,
      Curve::Exponential => (2.0_f32.powf(Self::CURVE_STEEPNESS * position) - 1.0) / scale,
      Curve::Logarithmic => (1.0 + position * scale).log2() / Self::CURVE_STEEPNESS,
    };
    let min = self.range_min as f32;
    let max = self.range_max as f32;
    (min + (max - min) * shaped).round().clamp(0.0, u16::MAX as f32) as u16
  }
}

///
/// Writes control changes into a parameter of an effect.
/// Only holds a weak reference, so the observer becomes inactive once its effect is dropped.
///
pub struct EffectInputObserver {
  value_storage: Weak<AtomicU16>,
  mapping: ControlMapping,
}

impl EffectInputObserver {
  pub fn new(value_storage: &Arc<AtomicU16>, mapping: ControlMapping) -> Self {
    Self { value_storage: Arc::downgrade(value_storage), mapping }
  }

  fn store(&self, value: u16) {
    if let Some(value_storage) = self.value_storage.upgrade() {
      value_storage.store(value, Ordering::Relaxed);
    }
  }
}

impl ControlInputObserver for EffectInputObserver {
  fn update(&self, cc: &ControlChange) {
    // Triggered by the hardware/control thread.
    // The audio thread will pick this up on its next process cycle.
    self.store(self.mapping.map(cc.value));
  }

  fn touch(&self, touched: bool) {
    if !touched && self.mapping.release == Release::Return {
      self.store(self.mapping.base);
    }
  }

  fn is_active(&self) -> bool {
    self.value_storage.strong_count() > 0
  }
}

///
/// Switches an effect on and off when a touchable control is touched.
///
pub struct EffectGateObserver {
  active: Weak<AtomicBool>,
  mode: GateMode,
}

impl EffectGateObserver {
  pub fn new(active: &Arc<AtomicBool>, mode: GateMode) -> Self {
    Self { active: Arc::downgrade(active), mode }
  }
}

impl ControlInputObserver for EffectGateObserver {
  fn update(&self, _cc: &ControlChange) {}

  fn touch(&self, touched: bool) {
    let Some(active) = self.active.upgrade() else { return };
    match self.mode {
      GateMode::Momentary => active.store(touched, Ordering::Relaxed),
      GateMode::Toggle => if touched {
        active.fetch_xor(true, Ordering::Relaxed);
      },
    }
  }

  fn is_active(&self) -> bool {
    self.active.strong_count() > 0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn change(value: u16) -> ControlChange {
    ControlChange { control_id: "laser_0".to_string(), value }
  }

  #[test]
  fn linear_mapping_covers_the_range() {
    let mapping = ControlMapping { range_min: 1000, range_max: 3000, ..ControlMapping::full_range(0) };
    assert_eq!(mapping.map(0), 1000);
    assert_eq!(mapping.map(u16::MAX / 2), 2000);
    assert_eq!(mapping.map(u16::MAX), 3000);
  }

  #[test]
  fn inverted_range_reverses_direction() {
    let mapping = ControlMapping { range_min: 3000, range_max: 1000, ..ControlMapping::full_range(0) };
    assert_eq!(mapping.map(0), 3000);
    assert_eq!(mapping.map(u16::MAX), 1000);
  }

  #[test]
  fn curves_keep_endpoints_and_bend_the_middle() {
    for curve in [Curve::Linear, Curve::Exponential, Curve::Logarithmic] {
      let mapping = ControlMapping { curve, ..ControlMapping::full_range(0) };
      assert_eq!(mapping.map(0), 0, "{:?}", curve);
      assert_eq!(mapping.map(u16::MAX), u16::MAX, "{:?}", curve);
    }
    let middle = |curve| ControlMapping { curve, ..ControlMapping::full_range(0) }.map(u16::MAX / 2);
    assert!(middle(Curve::Exponential) < middle(Curve::Linear));
    assert!(middle(Curve::Logarithmic) > middle(Curve::Linear));
  }

  #[test]
  fn release_returns_to_base_value() {
    let storage = Arc::new(AtomicU16::new(500));
    let mapping = ControlMapping { release: Release::Return, ..ControlMapping::full_range(500) };
    let observer = EffectInputObserver::new(&storage, mapping);

    observer.touch(true);
    observer.update(&change(40000));
    assert_eq!(storage.load(Ordering::Relaxed), 40000);
    observer.touch(false);
    assert_eq!(storage.load(Ordering::Relaxed), 500);
  }

  #[test]
  fn hold_keeps_last_value_after_release() {
    let storage = Arc::new(AtomicU16::new(500));
    let observer = EffectInputObserver::new(&storage, ControlMapping::full_range(500));

    observer.update(&change(40000));
    observer.touch(false);
    assert_eq!(storage.load(Ordering::Relaxed), 40000);
  }

  #[test]
  fn momentary_gate_follows_touch() {
    let active = Arc::new(AtomicBool::new(false));
    let gate = EffectGateObserver::new(&active, GateMode::Momentary);

    gate.touch(true);
    assert!(active.load(Ordering::Relaxed));
    gate.touch(false);
    assert!(!active.load(Ordering::Relaxed));
  }

  #[test]
  fn toggle_gate_flips_on_every_touch() {
    let active = Arc::new(AtomicBool::new(false));
    let gate = EffectGateObserver::new(&active, GateMode::Toggle);

    gate.touch(true);
    gate.touch(false);
    assert!(active.load(Ordering::Relaxed));
    gate.touch(true);
    gate.touch(false);
    assert!(!active.load(Ordering::Relaxed));
  }
}
