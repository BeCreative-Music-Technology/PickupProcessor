use std::{thread, time};

pub fn sleep_micros(us: u64) {
  thread::sleep(time::Duration::from_micros(us));
}

pub fn sleep_millis(ms: u64) {
  thread::sleep(time::Duration::from_millis(ms));
}
