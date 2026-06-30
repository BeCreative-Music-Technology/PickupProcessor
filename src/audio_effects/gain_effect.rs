use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_input_observer::EffectInputObserver;
use crate::control_input::{ControlInputObserver};
use crate::error::Error;
use crate::logger;

pub struct GainEffect {
  mix: Arc<AtomicU16>,
  gain_value: Arc<AtomicU16>,
}

static LOG_ENVIRONMENT: &str = "GainEffect";

impl GainEffect {
  pub fn new(mix: u16, gain: u16) -> Self
  where
    Self: Sized
  {
    logger::info(LOG_ENVIRONMENT, "effect created");

    Self {
      mix: Arc::new(AtomicU16::new(mix)),
      gain_value: Arc::new(AtomicU16::new(gain)), // u16::MAX / 2
    }
  }

  fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
  }

  fn parse_gain(gain: u16) -> f32 {
    let u16_half = u16::MAX / 2;
    if gain < u16_half {
      effect_helper::map(gain, u16::MIN, u16_half, -6.0, 0.0) // -6 dB -> 0 dB
    } else if gain > u16_half {
      effect_helper::map(gain, u16_half, u16::MAX, 0.0, 18.0) // 0 dB -> 18 dB
    } else {
      1.0
    }
  }
}

impl AudioEffect for GainEffect {
  ///
  /// Processes the chunk of data given to the method and applies a gain.
  /// The gain is done by multiplying the signal with a set factor.
  /// Minimum gain value is -6db, highest gain value is +18db.
  ///
  /// `chunk` takes a `Vec<f32>` which contains the data to be processed by the gain effect.
  ///
  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]> {
    let gain = self.gain_value.load(Ordering::Relaxed);
    let gain_db = Self::parse_gain(gain);

    let mix = effect_helper::map(
      self.mix.load(Ordering::Relaxed),
      u16::MIN,
      u16::MAX,
      0.0,
      1.0
    );

    chunk.iter().map(|sample| {
      let processed = sample * Self::db_to_gain(gain_db);
      effect_helper::mix(*sample, processed, mix)
    }).collect()
  }

  ///
  /// Sets the new value of the gain effect.
  /// `u16: 0` is -6db, `u16: 327675` is +0db, `u16: 65,535` is +18db.
  ///
  /// `key` takes a `&str` with the key of the parameter to be changed, for example `"gain"`.
  ///
  /// `value` takes an 'u16' with the new value.
  ///
  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    match key {
      "mix" => self.mix.store(value, Ordering::Relaxed),
      "gain" => self.gain_value.store(value, Ordering::Relaxed),
      _ => return Err(Error::new("Unknown parameter"))
    };
    logger::info(LOG_ENVIRONMENT, &format!("set parameter {} to {}", key, value));
    Ok(())
  }

  /// The effect remains completely passive, simply producing an observer when asked
  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error> {
    let value = match key {
      "mix" => &self.mix,
      "gain" => &self.gain_value,
      _ => return Err(Error::new("Parameter not found")),
    };
    Ok(Arc::new(EffectInputObserver {
      value_storage: Arc::clone(value)
    }))
  }

  fn get_type(&self) -> &str {
    "gain"
  }
}
