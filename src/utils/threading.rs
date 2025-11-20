use std::sync::{Arc, Mutex, MutexGuard};

#[macro_export]
macro_rules! critical_section {
    ($semaphore:expr, $callback:block) => {
        let _guard = $semaphore.lock();
        $callback
    };
}

#[derive(Clone)]
pub struct Semaphore {
    mutex: Arc<Mutex<()>>,
}

impl Semaphore {
    pub fn new() -> Self {
        Semaphore { mutex: Arc::new(Mutex::new(())) }
    }

    pub fn lock(&self) -> MutexGuard<'_, ()> {
        self.mutex.lock().unwrap()
    }
}