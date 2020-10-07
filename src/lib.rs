extern crate alloc;
extern crate core;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(not(any(
feature = "vulkan",
feature = "dx12",
feature = "metal",
)))]
extern crate gfx_backend_empty as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal;
extern crate log;
extern crate winit;


pub mod engine;
pub mod ecs;
pub mod rendering;
mod engine_const;
