#[macro_export]
macro_rules! critical_section {
    ($log_mutex:expr, $callback:block) => {
        let _guard = $log_mutex.lock().unwrap();
        $callback
    };
}
