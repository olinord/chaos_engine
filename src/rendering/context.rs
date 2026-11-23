use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use vulkano::descriptor_set::{
    DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
    layout::DescriptorSetLayout,
};

use vulkano::{
    descriptor_set::allocator::StandardDescriptorSetAllocatorCreateInfo,
    device::{Device, physical::PhysicalDevice},
    memory::allocator::{FreeListAllocator, GenericMemoryAllocator},
    swapchain::{Swapchain, SwapchainCreateInfo},
};

use crate::rendering::{
    errors::{RenderError, RenderResultExt},
    sampler::{ChaosSampler, SamplerDescriptor},
    texture::ChaosTexture,
};

#[derive(Debug)]
pub struct SwapchainState {
    swapchain: Arc<Swapchain>,
    backbuffers: Vec<Arc<ChaosTexture>>,
}

#[derive(Debug)]
pub struct ChaosRenderContext {
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
    swapchain_state: RwLock<SwapchainState>,
    graphic_family_index: u32,
    mip_lod_bias: f32,
    frame_index: AtomicUsize,
    descriptor_set_cache: Mutex<HashMap<u64, Arc<DescriptorSet>>>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
}

impl ChaosRenderContext {
    pub fn new(
        physical_device: Arc<PhysicalDevice>,
        device: Arc<Device>,
        memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
        swapchain: Arc<Swapchain>,
        backbuffers: Vec<Arc<ChaosTexture>>,
        graphic_family_index: u32,
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
            }),
            graphic_family_index,
            mip_lod_bias,
            frame_index: AtomicUsize::new(0),
            descriptor_set_cache: Mutex::new(HashMap::new()),
            descriptor_set_allocator: Arc::new(StandardDescriptorSetAllocator::new(
                clone,
                StandardDescriptorSetAllocatorCreateInfo::default(),
            )),
        }
    }

    // A poisoned lock means another thread panicked while holding it; that panic is the
    // real error, so these just propagate it.
    fn swapchain_state(&self) -> std::sync::RwLockReadGuard<'_, SwapchainState> {
        self.swapchain_state
            .read()
            .expect("swapchain state lock poisoned")
    }

    fn descriptor_set_cache(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Arc<DescriptorSet>>> {
        self.descriptor_set_cache
            .lock()
            .expect("descriptor set cache lock poisoned")
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn physical_device(&self) -> Arc<PhysicalDevice> {
        self.physical_device.clone()
    }

    pub fn swapchain(&self) -> Arc<Swapchain> {
        self.swapchain_state().swapchain.clone()
    }

    pub fn memory_allocator(&self) -> Arc<GenericMemoryAllocator<FreeListAllocator>> {
        self.memory_allocator.clone()
    }

    pub fn graphic_family_index(&self) -> u32 {
        self.graphic_family_index
    }

    pub fn backbuffers(&self) -> Vec<Arc<ChaosTexture>> {
        self.swapchain_state().backbuffers.clone()
    }

    pub fn mip_lod_bias(&self) -> f32 {
        self.mip_lod_bias
    }

    pub fn frame_index(&self) -> usize {
        self.frame_index.load(Ordering::Relaxed)
    }

    pub fn increment_frame_index(&self) {
        self.frame_index.fetch_add(1, Ordering::Relaxed);
    }

    pub fn create_sampler(
        &self,
        descriptor: SamplerDescriptor,
    ) -> Result<Arc<ChaosSampler>, RenderError> {
        ChaosSampler::get(self, descriptor)
    }

    /// Recreate the swapchain and its image views with the given surface extent.
    /// The caller must ensure the GPU is not currently using the previous swapchain
    /// images (wait on all in-flight fences before calling).
    pub fn recreate_swapchain(&self, extent: [u32; 2]) -> Result<(), RenderError> {
        let mut state = self
            .swapchain_state
            .write()
            .expect("swapchain state lock poisoned");
        let create_info = SwapchainCreateInfo {
            image_extent: extent,
            ..state.swapchain.create_info()
        };
        let (new_swapchain, new_backbuffers) = state
            .swapchain
            .recreate(create_info)
            .context("recreate swapchain")?;
        let new_backbuffers = new_backbuffers
            .into_iter()
            .enumerate()
            .map(|(i, image)| {
                ChaosTexture::from_existing(format!("Backbuffer {}", i), image).map(Arc::new)
            })
            .collect::<Result<_, _>>()?;
        state.swapchain = new_swapchain;
        state.backbuffers = new_backbuffers;
        Ok(())
    }

    pub fn get_or_create_descriptor_set(
        &self,
        key: u64,
        set_layout: &Arc<DescriptorSetLayout>,
        write_fn: impl FnOnce() -> Vec<WriteDescriptorSet>,
    ) -> Result<Arc<DescriptorSet>, RenderError> {
        let mut cache = self.descriptor_set_cache();

        if let Some(set) = cache.get(&key) {
            return Ok(set.clone());
        }

        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            set_layout.clone(),
            write_fn(),
            [],
        )
        .context("create descriptor set")?;
        cache.insert(key, set.clone());
        Ok(set)
    }

    pub fn clear_descriptor_set_cache(&self) {
        self.descriptor_set_cache().clear();
    }
}
