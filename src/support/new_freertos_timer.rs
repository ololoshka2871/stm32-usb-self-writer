use freertos_rust::{DurationTicks, Timer};

pub fn new_freertos_timer<D, F>(duration: D, name: &str, f: F) -> Timer
where
    D: DurationTicks,
    F: Fn(Timer) + Send + 'static,
{
    Timer::new(duration)
        .set_name(name)
        .set_auto_reload(false)
        .create(f)
        .expect("Out of memory")
}
