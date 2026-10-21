use std::sync::Arc;

use vulkano::{
    Validated, VulkanError,
    device::{Device, physical::PhysicalDevice},
    image::ImageUsage,
    render_pass::RenderPass,
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
};

use crate::rendering::texture::ChaosTexture;

pub type SwapchainAndImages = (Arc<Swapchain>, Vec<Arc<ChaosTexture>>);

pub fn get_swapchain_and_backbuffers(
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    surface: Arc<Surface>,
    dimensions: [u32; 2],
) -> Result<SwapchainAndImages, Validated<VulkanError>> {
    let caps = physical_device
        .surface_capabilities(&surface, Default::default())
        .expect("failed to get surface capabilities");

    let composite_alpha = caps.supported_composite_alpha.into_iter().next().unwrap();
    let image_format = physical_device
        .surface_formats(&surface, Default::default())
        .unwrap()[0]
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
    )?;

    let mut backbuffers: Vec<Arc<ChaosTexture>> = Vec::new();
    for raw_backbuffer in raw_backbuffers {
        backbuffers.push(Arc::new(ChaosTexture::from_existing(
            format!("Backbuffer {}", backbuffers.len()),
            raw_backbuffer,
        )));
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
    .unwrap()
}
