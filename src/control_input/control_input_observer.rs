// <<struct>> ControlChange
#[derive(Debug, Clone)]
pub struct ControlChange {
    pub control_id: String,
    pub value: u16,
}

pub trait ControlInputObserver: Send + Sync {
    fn update(&self, cc: &ControlChange);

    /// Returning `false` lets the observable drop this observer, e.g. once the effect it controls is gone.
    fn is_active(&self) -> bool {
        true
    }
}
