use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicU16, Ordering};
use crate::control_input::{ControlChange, ControlInputObserver};

///
/// Writes control changes into a parameter of an effect.
/// Only holds a weak reference, so the observer becomes inactive once its effect is dropped.
///
pub struct EffectInputObserver {
  value_storage: Weak<AtomicU16>,
}

impl EffectInputObserver {
  pub fn new(value_storage: &Arc<AtomicU16>) -> Self {
    Self { value_storage: Arc::downgrade(value_storage) }
  }
}

impl ControlInputObserver for EffectInputObserver {
  fn update(&self, cc: &ControlChange) {
    // Triggered by the hardware/control thread.
    // The audio thread will pick this up on its next process cycle.
    if let Some(value_storage) = self.value_storage.upgrade() {
      value_storage.store(cc.value, Ordering::Relaxed);
    }
  }

  fn is_active(&self) -> bool {
    self.value_storage.strong_count() > 0
  }
}
