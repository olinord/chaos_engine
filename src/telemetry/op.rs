#[cfg(feature = "profiling")]
pub use tracy_client;

pub fn init() {
    let _ = tracy_client::Client::start();
}

#[macro_export]
macro_rules! trace_zone {
    ($name:expr) => {
        let _client = $crate::telemetry::op::tracy_client::span!($name, 3);
    };
}

#[macro_export]
macro_rules! frame_zone {
    ($name:expr) => {
        let _client = $crate::telemetry::op::tracy_client::non_continuous_frame!($name);
    };
}
