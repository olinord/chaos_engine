use std::sync::Arc;

use chaos_communicator::communicator::ChaosReceiver;
use vulkano::{
    Validated, ValidationError, VulkanError, VulkanLibrary,
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferExecFuture, CommandBufferUsage,
        PrimaryAutoCommandBuffer, RenderPassBeginInfo, RenderingAttachmentInfo, RenderingInfo,
        SubpassBeginInfo, SubpassContents, allocator::StandardCommandBufferAllocator,
    },
    device::{Device, DeviceCreateInfo, Queue, QueueCreateInfo, physical::PhysicalDevice},
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::{FreeListAllocator, GenericMemoryAllocator, StandardMemoryAllocator},
    pipeline::graphics::viewport::Viewport,
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
    ecs::world::ChaosWorld,
    rendering::{adapters::select_physical_device, swapchain::get_swapchain_and_backbuffers},
};

pub type Fence = FenceSignalFuture<
    PresentFuture<CommandBufferExecFuture<JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture>>>,
>;

#[allow(unused)]
pub struct ChaosRenderSystem {
    physical_device: Option<Arc<PhysicalDevice>>,
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    current_frame: u128,
    current_buffer: u32,
    swapchain: Arc<Swapchain>,
    fences: Vec<Option<Arc<Fence>>>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
    add_render_component: ChaosReceiver,
    viewport: Viewport,
}

trait ChaosRenderableTrait {
    fn add_to_command_buffer(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), Box<ValidationError>>;

    fn initialize(
        &self,
        device: Arc<Device>,
        memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
        vieport: &Viewport,
    ) -> Result<(), &'static str>;
}

pub struct ChaosRenderableContainer {
    renderable: Arc<dyn ChaosRenderableTrait>,
}

impl ChaosRenderSystem {
    pub fn new(
        display_handle: &DisplayHandle,
        window: Arc<Window>,
        add_render_component: ChaosReceiver,
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
                ..Default::default()
            },
        ) {
            Ok(r) => r,
            Err(e) => panic!("failed to create device: {e}"),
        };

        println!("Device created successfully");
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

        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
            device.clone(),
            Default::default(),
        ));
        StandardCommandBufferAllocator::new(device.clone(), Default::default());

        let fences: Vec<Option<Arc<Fence>>> = vec![None; 3];

        ChaosRenderSystem {
            physical_device: Some(physical_device),
            device: Some(device),
            queue: Some(queue),
            current_frame: 0,
            current_buffer: 0,
            swapchain,
            fences,
            command_buffer_allocator,
            memory_allocator,
            add_render_component,
            viewport,
        }
    }

    pub fn start_frame(&mut self) -> AutoCommandBufferBuilder<PrimaryAutoCommandBuffer> {
        let queue = self.queue.as_ref().unwrap();
        let mut current_builder = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit,
        )
        .unwrap();

        let rendering_info = RenderingInfo {
            render_area_extent: [
                self.viewport.extent[0] as u32,
                self.viewport.extent[1] as u32,
            ],
            ..Default::default()
        };
        current_builder.begin_rendering(rendering_info).unwrap();
        return current_builder;
    }

    pub fn render(
        &self,
        container: Vec<&ChaosRenderableContainer>,
        buffer_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        for renderable in container {
            match renderable.renderable.add_to_command_buffer(buffer_builder) {
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
        buffer_builder.end_render_pass(Default::default()).unwrap();
        let command_buffer = buffer_builder.build().unwrap();

        let (image_i, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    return;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };

        if suboptimal {
            println!("Swapchain is suboptimal");
        }

        // wait for the fence related to this image to finish (normally this would be the oldest fence)
        if let Some(image_fence) = &self.fences[image_i as usize] {
            image_fence.wait(None).unwrap();
        }

        let previous_future = match self.fences[self.current_frame as usize].clone() {
            // Create a NowFuture
            None => {
                let mut now = sync::now(self.device.clone().unwrap());
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
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_i),
            )
            .then_signal_fence_and_flush();

        self.fences[image_i as usize] = match future.map_err(Validated::unwrap) {
            Ok(value) => Some(Arc::new(value)),
            Err(VulkanError::OutOfDate) => None,
            Err(e) => {
                println!("failed to flush future: {e}");
                None
            }
        };
        self.current_buffer = image_i;
    }

    pub fn update(&mut self, world: &mut ChaosWorld) {
        // iterate over the added and removed components
        // gather all entity ids from the add_render_component receiver
        let mut added_entities: Vec<&ChaosRenderableContainer> = vec![];
        loop {
            let message = self.add_render_component.receive();
            if message.is_none() {
                break;
            }
            let message = message.unwrap();
            let entity_id = message.get("entity_id").unwrap();
            added_entities.push(
                world
                    .component_manager
                    .get_component::<ChaosRenderableContainer>(entity_id)
                    .unwrap(),
            );
        }

        for entity in added_entities {
            match entity.renderable.initialize(
                self.device.clone().unwrap(),
                self.memory_allocator.clone(),
                &self.viewport,
            ) {
                Ok(()) => {}
                Err(e) => {
                    println!("Failed to initialize renderable: {e}");
                }
            }
        }
    }

    // pub fn create_command_buffer<T: BufferContents>(
    //     &self,
    //     buffer: Arc<CEBuffer<T>>,
    //     effect: Arc<CEEffect>,
    // ) -> Arc<PrimaryAutoCommandBuffer> {
    //     let device = self.device.as_ref().unwrap();
    //     let queue = self.queue.as_ref().unwrap();

    //     let command_buffer_allocator =
    //         StandardCommandBufferAllocator::new(device.clone(), Default::default());

    //     return get_command_buffers(
    //         &command_buffer_allocator,
    //         queue,
    //         &effect.pipeline,
    //         &self.framebuffers,
    //         &buffer.buffer,
    //     );
    // }
}
