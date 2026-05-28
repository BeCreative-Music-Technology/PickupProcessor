pub mod control_input_observer;
pub mod control_input;
pub mod observable_control_input;

// Re-export items so other packages can use `use crate::control_input::ControlChange;`
pub use control_input_observer::{ControlChange, ControlInputObserver};
pub use control_input::{ControlInput, LaserInput, PotentiometerInput};
pub use observable_control_input::ObservableControlInput;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU16, Ordering};
    use std::sync::Arc;

    struct MockObserver {
        pub last_value: AtomicU16,
    }

    impl ControlInputObserver for MockObserver {
        fn update(&self, cc: &ControlChange) {
            self.last_value.store(cc.value, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_observer_notification() {
        let observable = ObservableControlInput::new();
        let observer = Arc::new(MockObserver {
            last_value: AtomicU16::new(0),
        });

        observable.register(observer.clone());

        let change = ControlChange {
            control_id: "pot_volume".to_string(),
            value: 512,
            enabled: true,
        };
        observable.notify(&change);

        assert_eq!(observer.last_value.load(Ordering::SeqCst), 512);
    }

    #[test]
    fn test_observer_unregistration() {
        let observable = ObservableControlInput::new();
        let observer = Arc::new(MockObserver {
            last_value: AtomicU16::new(0),
        });

        observable.register(observer.clone());
        observable.unregister(observer.clone());

        let change = ControlChange {
            control_id: "laser_cutoff".to_string(),
            value: 1024,
            enabled: true,
        };
        observable.notify(&change);

        // The value should remain 0 because it was successfully unregistered
        assert_eq!(observer.last_value.load(Ordering::SeqCst), 0);
    }
}