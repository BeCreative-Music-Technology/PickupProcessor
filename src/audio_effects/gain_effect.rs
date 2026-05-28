use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::error::Error;

pub struct GainEffect {
  gain_value: u16,
}

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
    Self {
      gain_value: u16::MAX / 2,
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
    let u16_half = u16::MAX / 2;

    let gain_db: f32 = if self.gain_value < u16_half {
      effect_helper::map(self.gain_value, u16::MIN, u16_half, Self::MIN_GAIN_VALUE, 1.0)
    }
    else if self.gain_value > u16_half {
      effect_helper::map(self.gain_value, u16_half, u16::MAX, 1.0, Self::MAX_GAIN_VALUE)
    }
    else {
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
    match key {
      "gain" => { self.gain_value = value; Ok(()) },
      _ => Err(Error::new("Unknown parameter")),
    }
  }
}
