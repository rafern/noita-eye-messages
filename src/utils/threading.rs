use std::{sync::{Arc, Mutex, MutexGuard}, thread::{Builder, JoinHandle, available_parallelism, spawn}};

#[macro_export]
macro_rules! critical_section {
    ($semaphore:expr, $callback:block) => {
        let guard = $semaphore.lock();
        let x = $callback;
        drop(guard);
        x
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

pub struct Semaphore {
    mutex: Mutex<()>,
}

impl Semaphore {
    pub fn new() -> Self {
        Semaphore { mutex: Mutex::new(()) }
    }

    pub fn lock(&self) -> MutexGuard<'_, ()> {
        self.mutex.lock().unwrap()
    }
}

pub fn get_worker_slice<T>(max_value: T, worker_id: u32, worker_total: u32) -> (T, T) where
    T: TryFrom<u64> + Into<u64> + Copy,
    <T as TryFrom<u64>>::Error: std::fmt::Debug
{
    // TODO this is a really shitty method. improve it
    let min = (worker_id as u64 * (T::into(max_value) + 1)) / worker_total as u64;
    let max = ((worker_id + 1) as u64 * (T::into(max_value) + 1)) / worker_total as u64;
    (T::try_from(min).unwrap(), T::try_from(max - 1).unwrap())
}

pub fn get_parallelism() -> u32 {
    available_parallelism().unwrap_or(unsafe { std::num::NonZero::new_unchecked(1) }).get() as u32
}