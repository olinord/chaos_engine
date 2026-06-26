#[macro_use]
mod swizzle;

pub mod matrix;
pub mod quaternion;
mod vector;

pub use vector::{vec2::Vec2, vec3::Vec3, vec4::Vec4};
