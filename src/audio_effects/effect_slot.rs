use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::audio_effects::audio_effect::AudioEffect;

///
/// An effect in a bus' chain, with a switch that turns it on and off (e.g. while a laser string is touched).
/// Switching crossfades between the dry and processed signal, so it does not click.
/// The effect keeps processing while off, so delay lines and reverb tails stay in sync.
///
pub struct EffectSlot {
  effect: Box<dyn AudioEffect>,
  active: Arc<AtomicBool>,
  // 0.0 is fully bypassed, 1.0 is fully on
  fade: f32,
}

impl EffectSlot {
  /// How long switching an effect on or off takes.
  pub const FADE_SECONDS: f64 = 0.01;

  pub fn new(effect: Box<dyn AudioEffect>, active: bool) -> Self {
    Self {
      effect,
      active: Arc::new(AtomicBool::new(active)),
      fade: if active { 1.0 } else { 0.0 },
    }
  }

  /// Shared switch of this slot, for control inputs that turn the effect on and off.
  pub fn active(&self) -> Arc<AtomicBool> {
    self.active.clone()
  }

  pub fn effect(&self) -> &dyn AudioEffect {
    self.effect.as_ref()
  }

  pub fn effect_mut(&mut self) -> &mut dyn AudioEffect {
    self.effect.as_mut()
  }

  /// Change in `fade` per sample, so a full fade takes `FADE_SECONDS`.
  pub fn fade_step(sample_rate: usize) -> f32 {
    (1.0 / (Self::FADE_SECONDS * sample_rate as f64)) as f32
  }

  pub fn process(&mut self, sample: f32, fade_step: f32) -> f32 {
    let processed = self.effect.process_chunk(sample);

    let target = if self.active.load(Ordering::Relaxed) { 1.0 } else { 0.0 };
    if self.fade < target {
      self.fade = (self.fade + fade_step).min(target);
    } else if self.fade > target {
      self.fade = (self.fade - fade_step).max(target);
    }

    sample + (processed - sample) * self.fade
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::AtomicU16;
  use crate::error::Error;

  /// Effect that always outputs 1.0, so the crossfade is easy to see.
  struct ConstantEffect;

  impl AudioEffect for ConstantEffect {
    fn process_chunk(&mut self, _sample: f32) -> f32 { 1.0 }
    fn set_value(&mut self, _key: &str, _value: u16) -> Result<(), Error> { Ok(()) }
    fn parameter(&self, _key: &str) -> Result<Arc<AtomicU16>, Error> { Err(Error::new("none")) }
    fn get_type(&self) -> &str { "constant" }
  }

  #[test]
  fn switching_fades_between_dry_and_processed() {
    let sample_rate = 48000;
    let step = EffectSlot::fade_step(sample_rate);
    let fade_samples = (EffectSlot::FADE_SECONDS * sample_rate as f64).ceil() as usize;

    let mut slot = EffectSlot::new(Box::new(ConstantEffect), false);
    assert_eq!(slot.process(0.0, step), 0.0, "off passes the dry signal");

    slot.active().store(true, Ordering::Relaxed);
    let first = slot.process(0.0, step);
    assert!(first > 0.0 && first < 0.01, "switching on fades in, got {}", first);

    let last = (0..fade_samples).map(|_| slot.process(0.0, step)).last().unwrap();
    assert_eq!(last, 1.0, "fully on after the fade");
  }
}
