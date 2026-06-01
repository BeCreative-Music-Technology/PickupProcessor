use fundsp::Frame;
use fundsp::numeric_array::generic_array::GenericArray;
use fundsp::prelude32::{reverb_stereo, U2};
use crate::audio_effects::audio_effect::AudioEffect;
use crate::audio_effects::effect_helper;
use crate::error::Error;

pub struct ReverbEffect
{
  room_size: f32, // meters
  reverb_decay: f32, // seconds
  dampening: f32, // 0..1
}

impl AudioEffect for ReverbEffect{
  fn new() -> Self
  where
      Self: Sized
  {
    let room_size = 20.0;
    let reverb_decay = 2.0;
    let dampening = 0.0;

    Self { room_size, reverb_decay, dampening }
  }

  fn process_chunk(&mut self, chunk: Vec<f32>) -> Box<[f32]> {
    let mut reverb = reverb_stereo(self.room_size, self.reverb_decay, self.dampening);

    chunk
        .into_iter()
        .map(|sample| {
          reverb.tick(&Frame::new(GenericArray::<f32, U2>::from_array([sample, sample])))[0]
        })
        .collect::<Vec<f32>>()
        .into_boxed_slice()
  }

  fn set_value(&mut self, key: &str, value: u16) -> Result<(), Error> {
    match key {
      "room_size" => effect_helper::map(value, u16::MIN, u16::MAX, 10.0, 30.0),
      "reverb_decay" => effect_helper::map(value, u16::MIN, u16::MAX, 0.01, 20.0),
      "diffusion" => effect_helper::map(value, u16::MIN, u16::MAX, 0.0, 1.0),
      _ => return Err(Error::new("Unknown parameter")),
    };
    Ok(())
  }
}
