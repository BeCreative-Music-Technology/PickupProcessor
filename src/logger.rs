use colored::Colorize;
use crate::error::Error;

pub fn info(env: &str, info: &str) {
  println!("{}: {} - {}", "INFO".blue(), env, info);
}

pub fn log(env: &str, log: &str) {
  println!("{}: {} - {}", "LOG".blue(), env, log);
}

pub fn warn(env: &str, warn: &str) {
  println!("{}: {} - {}", "WARN".yellow(), env, warn);
}

pub fn error(env: &str, err: Error) {
  error_str(env, &err.message);
}

pub fn error_str(env: &str, err: &str) {
  println!("{}: {} - {}", "ERROR".red(), env, err);
}
