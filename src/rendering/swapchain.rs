use std::sync::Arc;

use vulkano::{
    Validated, VulkanError,
    device::{Device, physical::PhysicalDevice},
    image::{Image, ImageUsage},
    render_pass::RenderPass,
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
};

pub type SwapchainAndImages = (Arc<Swapchain>, Vec<Arc<Image>>);

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

    Swapchain::new(
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
