use std::sync::{atomic::AtomicBool, Arc};

pub struct GlobalStates {
    initialized: AtomicBool,
    pub shutting_down: Arc<AtomicBool>,
    pub shutted_down: AtomicBool,
}

impl GlobalStates {
    pub fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            shutting_down: Arc::new(AtomicBool::new(false)),
            shutted_down: AtomicBool::new(false),
        }
    }

    pub fn is_initialized(&self) -> bool {
        return self.initialized.load(std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_initialized(&self) {
        self.initialized
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_shutting_down(&self) -> bool {
        return self
            .shutting_down
            .load(std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_shutted_down(&self) {
        self.shutted_down
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
