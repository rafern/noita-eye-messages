use std::{sync::{Arc, Mutex, MutexGuard}, thread::{Builder, JoinHandle, spawn}};

#[macro_export]
macro_rules! critical_section {
    ($semaphore:expr, $callback:block) => {
        let _guard = $semaphore.lock();
        $callback
    };
}

pub struct AsyncTaskList {
    handles: Vec<JoinHandle<()>>,
}

impl AsyncTaskList {
    pub fn new() -> Self {
        AsyncTaskList { handles: Vec::new() }
    }

    pub fn wait(&mut self) {
        for handle in self.handles.drain(..) {
            handle.join().unwrap();
        }
    }

    pub fn add_async<F: FnOnce() + Send + 'static>(&mut self, task: F) {
        self.handles.push(spawn(task));
    }

    pub fn add_async_or_sync<F: FnMut() + Send + 'static>(&mut self, task: F) {
        // FIXME surely there's a better way to do this, right?
        let task_wrapper = Arc::new(Mutex::new(task));
        let task_wrapper_clone = task_wrapper.clone();

        match Builder::new().spawn(move || {
            (*task_wrapper_clone.lock().unwrap())();
        }) {
            Ok(handle) => {
                self.handles.push(handle);
            },
            Err(_) => {
                (*task_wrapper.lock().unwrap())();
            },
        }
    }
}

impl Drop for AsyncTaskList {
    fn drop(&mut self) { self.wait() }
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