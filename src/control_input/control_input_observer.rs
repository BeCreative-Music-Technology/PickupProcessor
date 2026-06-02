// <<struct>> ControlChange
#[derive(Debug, Clone)]
pub struct ControlChange {
    pub control_id: String,
    pub value: u16,
}

pub trait ControlInputObserver: Send + Sync {
    fn update(&self, cc: &ControlChange);
}