#[cfg(not(feature = "profiling"))]
pub mod no_op;

#[cfg(feature = "profiling")]
pub mod op;

#[cfg(not(feature = "profiling"))]
pub use no_op::init;

#[cfg(feature = "profiling")]
pub use op::init;
