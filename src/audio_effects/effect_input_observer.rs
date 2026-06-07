use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use crate::control_input::{ControlChange, ControlInputObserver};

pub struct EffectInputObserver {
  pub value_storage: Arc<AtomicU16>,
}

impl ControlInputObserver for EffectInputObserver {
  fn update(&self, cc: &ControlChange) {
    // Triggered by the hardware/control thread.
    // The audio thread will pick this up on its next process cycle.
    self.value_storage.store(cc.value, Ordering::Relaxed);
  }
}
