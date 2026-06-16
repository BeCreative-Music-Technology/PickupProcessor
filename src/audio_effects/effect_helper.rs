pub fn map(value: u16, range_min: u16, range_max: u16, value_min: f32, value_max: f32) -> f32 {
  let value_f32 = value as f32;
  let range_min_f32 = range_min as f32;
  let range_max_f32 = range_max as f32;
  let normalized = (value_f32 - range_min_f32) / (range_max_f32 - range_min_f32);
  normalized * (value_max - value_min) + value_min
}

pub fn mix(previous_value: f32, new_value: f32, mix_percentage: f32) -> f32 {
  previous_value * (1.0 - mix_percentage) + new_value * mix_percentage
}
