use vulkano::pipeline::graphics::viewport::{Scissor, Viewport};

/// Vulkan's clip-space Y points downward while OpenGL/GLM-style projection
/// matrices assume Y-up. Using a negative-height viewport tells the rasterizer
/// to flip Y for us, so callers can keep writing standard perspective /
/// orthographic matrices without per-shader compensation.
///
/// Side effect: front-face winding is inverted. All current pipelines use
/// `CullMode::None`; any future pipeline that enables back-face culling must
/// set `FrontFace::Clockwise`.
pub fn viewport(size: [f32; 2]) -> Viewport {
    Viewport {
        offset: [0.0, size[1]],
        extent: [size[0], -size[1]],
        depth_range: 0.0..=1.0,
    }
}

// flipped with offset
pub fn viewport_with_offset(size: [f32; 2], offset: [f32; 2]) -> Viewport {
    Viewport {
        offset: [offset[0], size[1] + offset[1]],
        extent: [size[0], -size[1]],
        depth_range: 0.0..=1.0,
    }
}

pub fn scissor(extent: [u32; 2], offset: [u32; 2]) -> Scissor {
    Scissor { offset, extent }
}
