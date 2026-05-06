use crate::audio_input::AudioInput;

pub struct RoutingDirector {
  audio_input: Vec<Box<dyn AudioInput>>,
}

impl RoutingDirector {}
