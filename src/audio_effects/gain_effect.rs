use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU8, Ordering};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::audio_effects::effect_input_observer::EffectInputObserver;
use crate::control_input::{ControlInputObserver};
use crate::error::Error;
use crate::logger;

pub struct GainEffect {
  // Wrap the parameter in an Arc<AtomicU16> so it can be shared safely
  gain_value: Arc<AtomicU16>,
  gain_id: String,
}

static GAIN_EFFECT_INCREMENTAL_ID: AtomicU8 = AtomicU8::new(0);
static LOG_ENVIRONMENT: String = String::from("GainEffect");

impl GainEffect {
  const MIN_GAIN_VALUE: f32 = -6.0; // -6db
  const MAX_GAIN_VALUE: f32 = 18.0; // +18db

  fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
  }
}

impl AudioEffect for GainEffect {
  fn new() -> Self
  where
      Self: Sized
  {
    let gain_id = format!("gain_{}", GAIN_EFFECT_INCREMENTAL_ID.fetch_add(1, Ordering::Relaxed));

    logger::info(&LOG_ENVIRONMENT, &format!("{} created", gain_id));
    
    Self {
      gain_value: Arc::new(AtomicU16::new(u16::MAX / 2)),
      gain_id,
    }
  }

  ///
  /// Processes the chunk of data given to the method and applies a gain.
  /// The gain is done by multiplying the signal with a set factor.
  /// Minimum gain value is -6db, highest gain value is +18db.
  ///
  /// `chunk` takes a `Vec<f32>` which contains the data to be processed by the gain effect.
  ///
  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]> {
    // Safely pull the current value exactly as it is right now
    let current_gain = self.gain_value.load(Ordering::Relaxed);

    let u16_half = u16::MAX / 2;
    let gain_db: f32 = if current_gain < u16_half {
      effect_helper::map(current_gain, u16::MIN, u16_half, Self::MIN_GAIN_VALUE, 1.0)
    } else if current_gain > u16_half {
      effect_helper::map(current_gain, u16_half, u16::MAX, 1.0, Self::MAX_GAIN_VALUE)
    } else {
      1.0
    };

    chunk.iter().map(|sample| {
      sample * Self::db_to_gain(gain_db)
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
    if key == "gain" {
      self.gain_value.store(value, Ordering::Relaxed);
    } else {
      return Err(Error::new("Unknown parameter"))
    }
    logger::info(&LOG_ENVIRONMENT, &format!("set parameter {} to {}", key, value));
    Ok(())
  }

  /// The effect remains completely passive, simply producing an observer when asked
  fn get_control_observer(&mut self, key: &str) -> Result<Arc<dyn ControlInputObserver>, Error> {
    match key {
      "gain" => {
        // Create an observer holding a cloned pointer to our atomic value
        let observer = Arc::new(EffectInputObserver {
          value_storage: Arc::clone(&self.gain_value),
        });
        Ok(observer)
      },
      _ => Err(Error::new("Parameter not found")),
    }
  }

  fn id(&self) -> &str {
    self.gain_id.as_str()
  }
}
