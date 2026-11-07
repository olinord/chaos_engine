use std::sync::Arc;

use vulkano::{
    device::{Device, physical::PhysicalDevice},
    image::ImageUsage,
    render_pass::RenderPass,
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
};

use crate::rendering::errors::{RenderError, RenderResultExt};
use crate::rendering::texture::ChaosTexture;

pub type SwapchainAndImages = (Arc<Swapchain>, Vec<Arc<ChaosTexture>>);

pub fn get_swapchain_and_backbuffers(
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    surface: Arc<Surface>,
    dimensions: [u32; 2],
) -> Result<SwapchainAndImages, RenderError> {
    let caps = physical_device
        .surface_capabilities(&surface, Default::default())
        .context("query surface capabilities")?;

    // Vulkan guarantees at least one supported composite alpha mode and one surface format.
    let composite_alpha = caps
        .supported_composite_alpha
        .into_iter()
        .next()
        .expect("surface supports no composite alpha mode");
    let image_format = physical_device
        .surface_formats(&surface, Default::default())
        .context("query surface formats")?
        .first()
        .expect("surface supports no formats")
        .0;

    let (swapchain, raw_backbuffers) = Swapchain::new(
        device.clone(),
        surface,
        SwapchainCreateInfo {
            min_image_count: caps.min_image_count,
            image_format,
            image_extent: dimensions,
            image_usage: ImageUsage::COLOR_ATTACHMENT,
            composite_alpha,
            ..Default::default()
        },
    )
    .context("create swapchain")?;

    let mut backbuffers: Vec<Arc<ChaosTexture>> = Vec::new();
    for raw_backbuffer in raw_backbuffers {
        backbuffers.push(Arc::new(ChaosTexture::from_existing(
            format!("Backbuffer {}", backbuffers.len()),
            raw_backbuffer,
        )?));
    }
    Ok((swapchain, backbuffers))
}

pub fn get_render_pass(device: Arc<Device>, swapchain: &Arc<Swapchain>) -> Arc<RenderPass> {
    vulkano::single_pass_renderpass!(
        device,
        attachments: {
            color: {
                // Set the format the same as the swapchain.
                format: swapchain.image_format(),
                samples: 1,
                load_op: Clear,
                store_op: Store,
            },
        },
        pass: {
            color: [color],
            depth_stencil: {},
        },
    )
    .expect("single color attachment render pass is always valid")
}
