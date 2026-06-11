use colored::Colorize;
use crate::error::Error;

pub fn info(env: &str, info: &str) {
  println!("{} {} - {}", "INFO:".truecolor(247, 57, 216).bold(), env.bold(), info);
}

pub fn log(env: &str, log: &str) {
  println!("{} {} - {}", "LOG:".cyan().bold(), env.bold(), log);
}

pub fn warn(env: &str, warn: &str) {
  println!("{} {} - {}", "WARN:".yellow().bold(), env.bold(), warn);
}

pub fn error(env: &str, err: Error) {
  error_str(env, &err.message);
}

pub fn error_str(env: &str, err: &str) {
  println!("{} {} - {}", "ERROR:".red().bold(), env.bold(), err);
}
