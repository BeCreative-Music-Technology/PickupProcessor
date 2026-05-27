#[derive(Debug)]
pub struct Error {
  pub message: String,
}

impl Error {
  pub fn new(message: &str) -> Self {
    Self {message: String::from(message)}
  }
}
