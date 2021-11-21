use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// This struct is designed to change a referenced atomic value to false when it
/// goes out of scope for any reason.
pub struct RunningGuard {
    running: Arc<AtomicBool>,
}

impl RunningGuard {
    pub fn new(running: Arc<AtomicBool>) -> RunningGuard {
        RunningGuard { running }
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}
