use std::sync::{Arc, Mutex};
use super::control_input_observer::{ControlChange, ControlInputObserver};

pub struct ObservableControlInput {
    // Encapsulate the vector in a Mutex for internal thread-safety
    observers: Mutex<Vec<Arc<dyn ControlInputObserver>>>,
}

impl ObservableControlInput {
    pub fn new() -> Self {
        Self {
            observers: Mutex::new(Vec::new()),
        }
    }


    pub fn notify(&self, cc: &ControlChange) {
        if let Ok(subs) = self.observers.lock() {
            for subscriber in subs.iter() {
                subscriber.update(cc);
            }
        }
    }

    // Using &self allows registration even when wrapped inside an Arc
    pub fn register(&self, observer: Arc<dyn ControlInputObserver>) {
        if let Ok(mut subs) = self.observers.lock() {
            subs.push(observer);
        }
    }

    // Using &self allows unregistration safely from any thread
    pub fn unregister(&self, observer: Arc<dyn ControlInputObserver>) {
        if let Ok(mut subs) = self.observers.lock() {
            subs.retain(|sub| !Arc::ptr_eq(sub, &observer));
        }
    }
}