extern crate alloc;
extern crate core;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

extern crate gfx_hal;
extern crate log;
extern crate winit;
extern crate spirv_reflect;

extern crate bitflags;

pub use back::Backend as ChaosBackend;

// test stuff
#[cfg( any(test, feature="no_gpu"))]
extern crate gfx_backend_empty as back;
#[cfg(test)]
extern crate mockall;


pub mod engine;
pub mod ecs;
pub mod rendering;
pub mod engine_const;
pub mod commands;
pub mod input;