use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use super::control_input_observer::{ControlChange, ControlInputObserver};

/// Latest known state of a control input, used to show it live in the panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlState {
    /// Latest value, `None` until the control produced one.
    pub value: Option<u16>,
    pub touched: bool,
    /// `false` when the hardware behind the control could not be initialized.
    pub online: bool,
    /// Increases on every change, so readers can tell whether anything changed since they last looked.
    pub revision: u32,
}

pub struct ObservableControlInput {
    // Encapsulate the vector in a Mutex for internal thread-safety
    observers: Mutex<Vec<Arc<dyn ControlInputObserver>>>,
    // Stored as u32 so NO_VALUE, outside the u16 range, can mean "no value yet"
    value: AtomicU32,
    touched: AtomicBool,
    online: AtomicBool,
    revision: AtomicU32,
}

impl ObservableControlInput {
    const NO_VALUE: u32 = u32::MAX;

    pub fn new() -> Self {
        Self {
            observers: Mutex::new(Vec::new()),
            value: AtomicU32::new(Self::NO_VALUE),
            touched: AtomicBool::new(false),
            online: AtomicBool::new(true),
            revision: AtomicU32::new(0),
        }
    }

    pub fn notify(&self, cc: &ControlChange) {
        self.value.store(cc.value as u32, Ordering::Relaxed);
        self.revision.fetch_add(1, Ordering::Release);

        if let Ok(subs) = self.observers.lock() {
            for subscriber in subs.iter() {
                subscriber.update(cc);
            }
        }
    }

    pub fn notify_touch(&self, touched: bool) {
        self.touched.store(touched, Ordering::Relaxed);
        self.revision.fetch_add(1, Ordering::Release);

        if let Ok(subs) = self.observers.lock() {
            for subscriber in subs.iter() {
                subscriber.touch(touched);
            }
        }
    }

    pub fn set_online(&self, online: bool) {
        if self.online.swap(online, Ordering::Relaxed) != online {
            self.revision.fetch_add(1, Ordering::Release);
        }
    }

    pub fn state(&self) -> ControlState {
        let revision = self.revision.load(Ordering::Acquire);
        let value = self.value.load(Ordering::Relaxed);
        ControlState {
            value: (value != Self::NO_VALUE).then_some(value as u16),
            touched: self.touched.load(Ordering::Relaxed),
            online: self.online.load(Ordering::Relaxed),
            revision,
        }
    }

    // Using &self allows registration even when wrapped inside an Arc
    pub fn register(&self, observer: Arc<dyn ControlInputObserver>) {
        if let Ok(mut subs) = self.observers.lock() {
            // Prune observers of effects that no longer exist, so re-sending a chain does not leak them
            subs.retain(|sub| sub.is_active());
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
