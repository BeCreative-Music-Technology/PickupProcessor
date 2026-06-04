use fundsp::prelude32::{lowpass_hz, An, FixedSvf, LowpassMode};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::error::Error;

pub struct LowPassFilterEffect {
  frequency: f32,
  q_factor: f32,
  filter: An<FixedSvf<f32, LowpassMode<f32>>>,
}

impl AudioEffect for LowPassFilterEffect {
  fn new() -> Self
  where
      Self: Sized
  {
    Self {
      frequency: 1000.0,
      q_factor: 0.707,
      filter: lowpass_hz(1000.0, 0.707),
    }
  }

  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]> {
    chunk
        .into_iter()
        .map(|sample| {
          self.filter.tick(&[sample].into())[0]
        })
        .collect::<Vec<f32>>()
        .into_boxed_slice()
  }

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    match key {
      "frequency" => {
        self.frequency = effect_helper::map(value, u16::MIN, u16::MAX, 20.0, 20000.0);
      },
      "q_factor" => {
        self.q_factor = if value < u16::MAX / 2 {
          effect_helper::map(value, u16::MIN, u16::MAX / 2, 0.3, 0.707)
        } else {
          effect_helper::map(value, u16::MAX / 2, u16::MAX, 0.707, 10.0)
        };
      },
      _ => return Err(Error::new("Unknown parameter")),
    };
    self.filter = lowpass_hz(self.frequency, self.q_factor);
    Ok(())
  }
}
