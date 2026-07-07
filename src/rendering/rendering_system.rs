use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use chaos_communicator::communicator::ChaosReceiver;
use log::debug;
use vulkano::{
    Validated, ValidationError, VulkanError, VulkanLibrary,
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferExecFuture, CommandBufferUsage,
        PrimaryAutoCommandBuffer, RenderingAttachmentInfo, RenderingInfo,
        allocator::StandardCommandBufferAllocator,
    },
    device::{
        Device, DeviceCreateInfo, DeviceFeatures, Queue, QueueCreateInfo, physical::PhysicalDevice,
    },
    format::ClearValue,
    image::view::ImageView,
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::{FreeListAllocator, GenericMemoryAllocator, StandardMemoryAllocator},
    pipeline::graphics::viewport::{Scissor, Viewport},
    render_pass::{AttachmentLoadOp, AttachmentStoreOp},
    swapchain::{
        self, PresentFuture, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
        SwapchainPresentInfo,
    },
    sync::{
        self, GpuFuture,
        future::{FenceSignalFuture, JoinFuture},
    },
};
use winit::{raw_window_handle::DisplayHandle, window::Window};

use crate::{
    ecs::{EntityID, world::ChaosWorld},
    rendering::{adapters::select_physical_device, swapchain::get_swapchain_and_backbuffers},
};

pub type Fence = FenceSignalFuture<
    PresentFuture<CommandBufferExecFuture<JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture>>>,
>;

#[derive(Debug)]
struct SwapchainState {
    swapchain: Arc<Swapchain>,
    image_views: Vec<Arc<ImageView>>,
    viewport: Viewport,
}

#[derive(Debug)]
pub struct ChaosRenderContext {
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
    swapchain_state: RwLock<SwapchainState>,
}

impl ChaosRenderContext {
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

    pub fn image_views(&self) -> Vec<Arc<ImageView>> {
        self.swapchain_state.read().unwrap().image_views.clone()
    }

    /// Recreate the swapchain and its image views with the given surface extent.
    /// The caller must ensure the GPU is not currently using the previous swapchain
    /// images (wait on all in-flight fences before calling).
    fn recreate_swapchain(&self, extent: [u32; 2]) -> Result<(), Validated<VulkanError>> {
        let mut state = self.swapchain_state.write().unwrap();
        let create_info = SwapchainCreateInfo {
            image_extent: extent,
            ..state.swapchain.create_info()
        };
        let (new_swapchain, new_images) = state.swapchain.recreate(create_info)?;
        let new_image_views = new_images
            .into_iter()
            .map(|image| {
                ImageView::new_default(image).expect("failed to create swapchain image view")
            })
            .collect();
        state.swapchain = new_swapchain;
        state.image_views = new_image_views;
        state.viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [extent[0] as f32, extent[1] as f32],
            depth_range: 0.0..=1.0,
        };
        Ok(())
    }
}

#[allow(unused)]
pub struct ChaosRenderSystem {
    render_context: Arc<ChaosRenderContext>,
    queue: Option<Arc<Queue>>,
    current_frame: u128,
    current_buffer: u32,
    current_acquire_future: Option<SwapchainAcquireFuture>,
    fences: Vec<Option<Fence>>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    add_render_component: ChaosReceiver,
    directories: HashMap<PathBuf, PathBuf>,
    pending_resize: Option<[u32; 2]>,
}

pub trait ChaosRenderableTrait {
    fn initialize(
        &mut self,
        world: &ChaosWorld,
        entity_id: EntityID,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str>;

    fn update(
        &mut self,
        world: &ChaosWorld,
        entity_id: EntityID,
        render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str>;

    fn add_to_command_buffer(
        &self,
        world: &ChaosWorld,
        entity_id: EntityID,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), Box<ValidationError>>;
}

pub struct ChaosRenderableContainer {
    renderable: RefCell<Box<dyn ChaosRenderableTrait>>,
}

impl ChaosRenderableContainer {
    pub fn new<T>(renderable: T) -> Self
    where
        T: ChaosRenderableTrait + 'static,
    {
        Self {
            renderable: RefCell::new(Box::new(renderable)),
        }
    }
}

impl ChaosRenderSystem {
    pub fn new(
        display_handle: &DisplayHandle,
        window: Arc<Window>,
        add_render_component: ChaosReceiver,
        directories: &HashMap<PathBuf, PathBuf>,
    ) -> ChaosRenderSystem {
        let library = VulkanLibrary::new().expect("no local Vulkan library/DLL");

        let required_extensions = match Surface::required_extensions(display_handle) {
            Ok(ext) => ext,
            Err(_) => panic!("Couldn't get required surface extensions"),
        };

        let instance = match Instance::new(
            library,
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: required_extensions,
                ..Default::default()
            },
        ) {
            Ok(instance) => instance,
            Err(err) => panic!("Failed to create instance: {}", err),
        };

        let surface = match Surface::from_window(instance.clone(), window.clone()) {
            Ok(surface) => surface,
            Err(err) => panic!("Failed to create surface: {}", err),
        };

        let device_extensions = vulkano::device::DeviceExtensions {
            khr_swapchain: true,
            khr_dynamic_rendering: true,
            ..vulkano::device::DeviceExtensions::empty()
        };
        let (physical_device, queue_family_index) =
            select_physical_device(&instance, &surface, &device_extensions);

        let (device, mut queues) = match Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                // here we pass the desired queue family to use by index
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                enabled_extensions: device_extensions,
                enabled_features: DeviceFeatures {
                    dynamic_rendering: true,
                    triangle_fans: true,
                    ..DeviceFeatures::empty()
                },
                ..Default::default()
            },
        ) {
            Ok(r) => r,
            Err(e) => panic!("failed to create device: {e}"),
        };

        debug!("Device created successfully");
        let queue = queues.next().unwrap();

        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

        let inner_size = window.inner_size();
        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [inner_size.width as f32, inner_size.height as f32],
            depth_range: 0.0..=1.0,
        };
        let (swapchain, backbuffers) = match get_swapchain_and_backbuffers(
            physical_device.clone(),
            device.clone(),
            surface.clone(),
            [inner_size.width, inner_size.height],
        ) {
            Ok(r) => r,
            Err(e) => {
                panic!("failed to create swapchain: {e:?}")
            }
        };
        let image_views = backbuffers
            .into_iter()
            .map(|image| {
                ImageView::new_default(image).expect("failed to create swapchain image view")
            })
            .collect();

        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
            device.clone(),
            Default::default(),
        ));
        StandardCommandBufferAllocator::new(device.clone(), Default::default());

        let fences: Vec<Option<Fence>> = (0..3).map(|_| None).collect();

        let render_context = Arc::new(ChaosRenderContext {
            physical_device: physical_device.clone(),
            device: device.clone(),
            memory_allocator: memory_allocator.clone(),
            swapchain_state: RwLock::new(SwapchainState {
                swapchain,
                image_views,
                viewport,
            }),
        });

        ChaosRenderSystem {
            render_context: render_context.clone(),
            queue: Some(queue),
            current_frame: 0,
            current_buffer: 0,
            current_acquire_future: None,
            fences,
            command_buffer_allocator,
            add_render_component,
            directories: directories.clone(),
            pending_resize: None,
        }
    }

    pub fn start_frame(&mut self) -> Option<AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>> {
        if let Some(extent) = self.pending_resize.take() {
            if let Err(e) = self.recreate_swapchain_now(extent) {
                log::warn!("Failed to recreate swapchain: {e:?}");
                self.pending_resize = Some(extent);
                return None;
            }
        }

        let (image_i, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.render_context.swapchain(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    self.pending_resize = Some(self.render_context.swapchain().image_extent());
                    return None;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };

        if suboptimal {
            // Render this frame anyway, but recreate the swapchain before the next one.
            self.pending_resize = Some(self.render_context.swapchain().image_extent());
        }

        if let Some(image_fence) = &self.fences[image_i as usize] {
            image_fence.wait(None).unwrap();
        }

        self.current_buffer = image_i;
        self.current_acquire_future = Some(acquire_future);

        let queue = self.queue.as_ref().unwrap();
        let mut current_builder = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit,
        )
        .unwrap();

        let image_views = self.render_context.image_views();
        let mut color_attachment =
            RenderingAttachmentInfo::image_view(image_views[image_i as usize].clone());
        color_attachment.load_op = AttachmentLoadOp::Clear;
        color_attachment.store_op = AttachmentStoreOp::Store;
        color_attachment.clear_value = Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0]));

        let viewport_extent = self.render_context.viewport().extent;
        let rendering_info = RenderingInfo {
            render_area_extent: [viewport_extent[0] as u32, viewport_extent[1] as u32],
            color_attachments: vec![Some(color_attachment)],
            ..Default::default()
        };
        current_builder.begin_rendering(rendering_info).unwrap();

        // Push the current viewport/scissor to the command buffer so pipelines
        // built with dynamic viewport state (see `EffectFactory`) pick up the
        // current window size after a resize.
        let dynamic_viewport = self.render_context.viewport();
        current_builder
            .set_viewport(0, std::iter::once(dynamic_viewport).collect())
            .unwrap();
        current_builder
            .set_scissor(
                0,
                std::iter::once(Scissor {
                    offset: [0, 0],
                    extent: [viewport_extent[0] as u32, viewport_extent[1] as u32],
                })
                .collect(),
            )
            .unwrap();
        Some(current_builder)
    }

    pub fn render(
        &self,
        container: Vec<(EntityID, &ChaosRenderableContainer)>,
        buffer_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        world: &ChaosWorld,
    ) {
        for (entity_id, renderable) in container {
            match renderable.renderable.borrow().add_to_command_buffer(
                world,
                entity_id,
                buffer_builder,
            ) {
                Ok(()) => {}
                Err(e) => {
                    println!("Failed to add entity {} to command buffer: {e}", entity_id);
                }
            }
        }
    }

    pub fn end_frame(
        &mut self,
        buffer_builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        let mut buffer_builder = buffer_builder;
        buffer_builder.end_rendering().unwrap();
        let command_buffer = buffer_builder.build().unwrap();
        let image_i = self.current_buffer;
        let acquire_future = self
            .current_acquire_future
            .take()
            .expect("end_frame called before start_frame acquired a swapchain image");

        let previous_future = match self.fences[image_i as usize].take() {
            // Create a NowFuture
            None => {
                let mut now = sync::now(self.render_context.device.clone());
                now.cleanup_finished();

                now.boxed()
            }
            // Use the existing FenceSignalFuture
            Some(fence) => fence.boxed(),
        };

        let future = previous_future
            .join(acquire_future)
            .then_execute(self.queue.clone().unwrap(), command_buffer)
            .unwrap()
            .then_swapchain_present(
                self.queue.clone().unwrap(),
                SwapchainPresentInfo::swapchain_image_index(
                    self.render_context.swapchain(),
                    image_i,
                ),
            )
            .then_signal_fence_and_flush();

        self.fences[image_i as usize] = match future.map_err(Validated::unwrap) {
            Ok(value) => Some(value),
            Err(VulkanError::OutOfDate) => {
                self.pending_resize = Some(self.render_context.swapchain().image_extent());
                None
            }
            Err(e) => {
                println!("failed to flush future: {e}");
                None
            }
        };
        self.current_buffer = image_i;
        self.current_frame = self.current_frame.wrapping_add(1);
    }

    pub fn update(&mut self, world: &ChaosWorld) {
        // initialize the added components
        // and update the existing components
        let mut added_entity_ids: HashSet<EntityID> = HashSet::new();
        loop {
            let message = self.add_render_component.receive();
            if message.is_none() {
                break;
            }
            let message = message.unwrap();
            let entity_id: EntityID = message.get("entity_id").unwrap();
            added_entity_ids.insert(entity_id);
        }

        let all_renderables = world
            .get_all_components_of_type::<ChaosRenderableContainer>()
            .unwrap();

        for (entity_id, renderable) in all_renderables {
            let mut renderable = renderable.renderable.borrow_mut();
            if added_entity_ids.contains(&entity_id) {
                renderable
                    .initialize(world, entity_id, &self.render_context.clone())
                    .unwrap();
            }

            renderable
                .update(world, entity_id, &self.render_context.clone())
                .unwrap();
        }
    }

    pub fn render_context(&self) -> &Arc<ChaosRenderContext> {
        &self.render_context
    }

    /// Queue a swapchain recreation to happen at the start of the next frame.
    /// Zero-sized extents (e.g. from a minimized window) are ignored.
    pub fn request_resize(&mut self, extent: [u32; 2]) {
        if extent[0] == 0 || extent[1] == 0 {
            return;
        }
        self.pending_resize = Some(extent);
    }

    fn recreate_swapchain_now(
        &mut self,
        extent: [u32; 2],
    ) -> Result<(), Validated<VulkanError>> {
        // Wait for any in-flight frames before retiring the current swapchain.
        for fence_slot in self.fences.iter_mut() {
            if let Some(fence) = fence_slot.take() {
                let _ = fence.wait(None);
            }
        }
        self.render_context.recreate_swapchain(extent)
    }
}
