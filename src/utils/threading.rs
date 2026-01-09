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

// TODO remove this once input slices are implemented. this doesn't belong here,
//      it's specific to the ARX benchmark cipher
pub fn get_worklet_slice<T>(max_value: T, worklet_id: u32, worklet_total: u32) -> (T, T) where
    T: TryFrom<usize> + Into<usize> + Copy,
    <T as TryFrom<usize>>::Error: std::fmt::Debug
{
    let min = (worklet_id as usize * (T::into(max_value) + 1)) / worklet_total as usize;
    let max = ((worklet_id + 1) as usize * (T::into(max_value) + 1)) / worklet_total as usize;
    (T::try_from(min).unwrap(), T::try_from(max - 1).unwrap())
}

pub fn get_parallelism() -> u32 {
    available_parallelism().unwrap_or(std::num::NonZero::new(1).unwrap()).get() as u32
}