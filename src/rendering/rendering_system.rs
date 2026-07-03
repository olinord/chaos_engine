use std::{collections::HashMap, path::PathBuf, sync::Arc};

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
    pipeline::graphics::viewport::Viewport,
    render_pass::{AttachmentLoadOp, AttachmentStoreOp},
    swapchain::{
        self, PresentFuture, Surface, Swapchain, SwapchainAcquireFuture, SwapchainPresentInfo,
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
pub struct ChaosRenderContext {
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    swapchain: Arc<Swapchain>,
    image_views: Vec<Arc<ImageView>>,
    memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
    viewport: Viewport,
}

impl ChaosRenderContext {
    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn physical_device(&self) -> Arc<PhysicalDevice> {
        self.physical_device.clone()
    }

    pub fn swapchain(&self) -> Arc<Swapchain> {
        self.swapchain.clone()
    }

    pub fn memory_allocator(&self) -> Arc<GenericMemoryAllocator<FreeListAllocator>> {
        self.memory_allocator.clone()
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport.clone()
    }

    pub fn image_views(&self) -> Vec<Arc<ImageView>> {
        self.image_views.clone()
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
}

pub trait ChaosRenderableTrait {
    fn initialize(&mut self, render_context: &Arc<ChaosRenderContext>) -> Result<(), &'static str>;
    fn add_to_command_buffer(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        world: &ChaosWorld,
        entity: &EntityID,
    ) -> Result<(), Box<ValidationError>>;
}

pub struct ChaosRenderableContainer {
    renderable: Arc<dyn ChaosRenderableTrait>,
}

impl ChaosRenderableContainer {
    pub fn new<T>(renderable: T) -> Self
    where
        T: ChaosRenderableTrait + 'static,
    {
        Self {
            renderable: Arc::new(renderable),
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
            swapchain,
            image_views,
            memory_allocator: memory_allocator.clone(),
            viewport,
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
        }
    }

    pub fn start_frame(&mut self) -> Option<AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>> {
        let (image_i, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.render_context.swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    return None;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };

        if suboptimal {
            println!("Swapchain is suboptimal");
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

        let mut color_attachment = RenderingAttachmentInfo::image_view(
            self.render_context.image_views[image_i as usize].clone(),
        );
        color_attachment.load_op = AttachmentLoadOp::Clear;
        color_attachment.store_op = AttachmentStoreOp::Store;
        color_attachment.clear_value = Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0]));

        let rendering_info = RenderingInfo {
            render_area_extent: [
                self.render_context.viewport.extent[0] as u32,
                self.render_context.viewport.extent[1] as u32,
            ],
            color_attachments: vec![Some(color_attachment)],
            ..Default::default()
        };
        current_builder.begin_rendering(rendering_info).unwrap();
        Some(current_builder)
    }

    pub fn render(
        &self,
        container: Vec<(EntityID, &ChaosRenderableContainer)>,
        buffer_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        world: &ChaosWorld,
    ) {
        for (entity, renderable) in container {
            match renderable
                .renderable
                .add_to_command_buffer(buffer_builder, world, &entity)
            {
                Ok(()) => {}
                Err(e) => {
                    println!("Failed to add to command buffer: {e}");
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
                    self.render_context.swapchain.clone(),
                    image_i,
                ),
            )
            .then_signal_fence_and_flush();

        self.fences[image_i as usize] = match future.map_err(Validated::unwrap) {
            Ok(value) => Some(value),
            Err(VulkanError::OutOfDate) => None,
            Err(e) => {
                println!("failed to flush future: {e}");
                None
            }
        };
        self.current_buffer = image_i;
        self.current_frame = self.current_frame.wrapping_add(1);
    }

    pub fn update(&mut self, world: &mut ChaosWorld) {
        // iterate over the added and removed components
        // gather all entity ids from the add_render_component receiver
        let mut added_entity_ids = vec![];
        loop {
            let message = self.add_render_component.receive();
            if message.is_none() {
                break;
            }
            let message = message.unwrap();
            let entity_id = message.get("entity_id").unwrap();
            added_entity_ids.push(entity_id);
        }
        for entity_id in added_entity_ids {
            if let Some(entity) = world.get_component_mut::<ChaosRenderableContainer>(entity_id) {
                if let Some(renderable) = Arc::get_mut(&mut entity.renderable) {
                    match renderable.initialize(&self.render_context.clone()) {
                        Ok(()) => {}
                        Err(e) => {
                            println!("Failed to initialize renderable: {}", e);
                        }
                    }
                }
            }
        }
    }

    pub fn render_context(&self) -> &Arc<ChaosRenderContext> {
        &self.render_context
    }
}
