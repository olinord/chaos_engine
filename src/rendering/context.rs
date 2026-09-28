use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};
use vulkano::descriptor_set::{
    DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
    layout::DescriptorSetLayout,
};

use vulkano::{
    Validated, VulkanError,
    descriptor_set::allocator::StandardDescriptorSetAllocatorCreateInfo,
    device::{Device, physical::PhysicalDevice},
    memory::allocator::{FreeListAllocator, GenericMemoryAllocator},
    pipeline::graphics::viewport::Viewport,
    swapchain::{Swapchain, SwapchainCreateInfo},
};

use crate::rendering::{
    image::ChaosImage,
    sampler::{ChaosSampler, SamplerDescriptor},
};

/// Vulkan's clip-space Y points downward while OpenGL/GLM-style projection
/// matrices assume Y-up. Using a negative-height viewport tells the rasterizer
/// to flip Y for us, so callers can keep writing standard perspective /
/// orthographic matrices without per-shader compensation.
///
/// Side effect: front-face winding is inverted. All current pipelines use
/// `CullMode::None`; any future pipeline that enables back-face culling must
/// set `FrontFace::Clockwise`.
fn flipped_viewport(width: f32, height: f32) -> Viewport {
    Viewport {
        offset: [0.0, height],
        extent: [width, -height],
        depth_range: 0.0..=1.0,
    }
}

#[derive(Debug)]
pub struct SwapchainState {
    swapchain: Arc<Swapchain>,
    backbuffers: Vec<Arc<ChaosImage>>,
    viewport: Viewport,
}

#[derive(Debug)]
pub struct ChaosRenderContext {
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
    swapchain_state: RwLock<SwapchainState>,
    mip_lod_bias: f32,
    frame_index: usize,
    descriptor_set_cache: Mutex<HashMap<u64, Arc<DescriptorSet>>>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
}

impl ChaosRenderContext {
    pub fn new(
        physical_device: Arc<PhysicalDevice>,
        device: Arc<Device>,
        memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
        swapchain: Arc<Swapchain>,
        backbuffers: Vec<Arc<ChaosImage>>,
        viewport_size: [f32; 2],
        mip_lod_bias: f32,
    ) -> ChaosRenderContext {
        let clone = device.clone();
        ChaosRenderContext {
            physical_device,
            device,
            memory_allocator,
            swapchain_state: RwLock::new(SwapchainState {
                swapchain,
                backbuffers,
                viewport: flipped_viewport(viewport_size[0], viewport_size[1]),
            }),
            mip_lod_bias,
            frame_index: 0,
            descriptor_set_cache: Mutex::new(HashMap::new()),
            descriptor_set_allocator: Arc::new(StandardDescriptorSetAllocator::new(
                clone,
                StandardDescriptorSetAllocatorCreateInfo::default(),
            )),
        }
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn physical_device(&self) -> Arc<PhysicalDevice> {
        self.physical_device.clone()
    }

    pub fn swapchain(&self) -> Arc<Swapchain> {
        self.swapchain_state.read().unwrap().swapchain.clone()
    }

    pub fn memory_allocator(&self) -> Arc<GenericMemoryAllocator<FreeListAllocator>> {
        self.memory_allocator.clone()
    }

    pub fn viewport(&self) -> Viewport {
        self.swapchain_state.read().unwrap().viewport.clone()
    }

    pub fn backbuffers(&self) -> Vec<Arc<ChaosImage>> {
        self.swapchain_state.read().unwrap().backbuffers.clone()
    }

    pub fn mip_lod_bias(&self) -> f32 {
        self.mip_lod_bias
    }

    pub fn frame_index(&self) -> usize {
        self.frame_index
    }

    pub fn increment_frame_index(&mut self) {
        self.frame_index += 1;
    }

    pub fn create_sampler(&self, descriptor: SamplerDescriptor) -> Arc<ChaosSampler> {
        ChaosSampler::get(self, descriptor)
    }

    /// Recreate the swapchain and its image views with the given surface extent.
    /// The caller must ensure the GPU is not currently using the previous swapchain
    /// images (wait on all in-flight fences before calling).
    pub fn recreate_swapchain(&self, extent: [u32; 2]) -> Result<(), Validated<VulkanError>> {
        let mut state = self.swapchain_state.write().unwrap();
        let create_info = SwapchainCreateInfo {
            image_extent: extent,
            ..state.swapchain.create_info()
        };
        let (new_swapchain, new_backbuffers) = state.swapchain.recreate(create_info)?;
        let new_backbuffers = new_backbuffers
            .into_iter()
            .enumerate()
            .map(|(i, image)| {
                Arc::new(ChaosImage::from_existing(
                    format!("Backbuffer {}", i),
                    image,
                    vulkano::image::ImageLayout::PresentSrc,
                ))
            })
            .collect();
        state.swapchain = new_swapchain;
        state.backbuffers = new_backbuffers;
        state.viewport = flipped_viewport(extent[0] as f32, extent[1] as f32);
        Ok(())
    }

    pub fn get_or_create_descriptor_set(
        &self,
        key: u64,
        set_layout: &Arc<DescriptorSetLayout>,
        write_fn: impl FnOnce() -> Vec<WriteDescriptorSet>,
    ) -> Result<Arc<DescriptorSet>, Validated<VulkanError>> {
        let mut cache = self.descriptor_set_cache.lock().unwrap();

        if let Some(set) = cache.get(&key) {
            return Ok(set.clone());
        }

        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            set_layout.clone(),
            write_fn(),
            [],
        )?;
        cache.insert(key, set.clone());
        Ok(set)
    }

    pub fn clear_descriptor_set_cache(&self) {
        self.descriptor_set_cache.lock().unwrap().clear();
    }
}
