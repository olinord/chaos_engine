use std::sync::Arc;

use vulkano::{
    Validated, VulkanError, VulkanLibrary,
    buffer::BufferContents,
    command_buffer::{
        CommandBufferExecFuture, PrimaryAutoCommandBuffer,
        allocator::StandardCommandBufferAllocator,
    },
    device::{Device, DeviceCreateInfo, Queue, QueueCreateInfo, physical::PhysicalDevice},
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::StandardMemoryAllocator,
    pipeline::graphics::{vertex_input::Vertex, viewport::Viewport},
    render_pass::Framebuffer,
    swapchain::{
        self, PresentFuture, Surface, Swapchain, SwapchainAcquireFuture, SwapchainPresentInfo,
    },
    sync::{
        self, GpuFuture,
        future::{FenceSignalFuture, JoinFuture},
    },
};
use winit::{raw_window_handle::DisplayHandle, window::Window};

use crate::rendering::{
    adapters::select_physical_device,
    buffer::{CEBufferBuilder, CEBufferMemoryType, CEBufferUsage},
    command_buffers::get_command_buffers,
    effect::{CEEffectBuilder, CEEffectType},
    swapchain::{get_framebuffers, get_render_pass, get_swapchain_and_backbuffers},
};
pub type Fence = FenceSignalFuture<
    PresentFuture<CommandBufferExecFuture<JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture>>>,
>;

#[allow(unused)]
#[derive(Clone)]
pub struct ChaosRenderSystem {
    physical_device: Option<Arc<PhysicalDevice>>,
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    current_frame: u128,
    current_buffer: u32,
    framebuffers: Vec<Arc<Framebuffer>>,
    command_buffers: Vec<Arc<PrimaryAutoCommandBuffer>>,
    swapchain: Arc<Swapchain>,
    fences: Vec<Option<Arc<Fence>>>,
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32B32A32_SFLOAT)]
    color: [f32; 4],
}

impl ChaosRenderSystem {
    pub fn new(display_handle: &DisplayHandle, window: Arc<Window>) -> ChaosRenderSystem {
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

        println!("Creating vertex buffer");
        let buffer = match CEBufferBuilder::new("vertex_buffer".into())
            .with_allocator(memory_allocator)
            .with_usage(CEBufferUsage::VertexBuffer)
            .with_memory_type(CEBufferMemoryType::PreferDevice)
            .with_memory_type(CEBufferMemoryType::HostSequentialWrite)
            .build(vec![
                MyVertex {
                    position: [-0.5, -0.5],
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                MyVertex {
                    position: [0.0, 0.5],
                    color: [0.0, 1.0, 0.0, 1.0],
                },
                MyVertex {
                    position: [0.5, -0.25],
                    color: [0.0, 0.0, 1.0, 1.0],
                },
            ]) {
            Ok(b) => b,
            Err(e) => panic!("failed to create vertex buffer: {e:?}"),
        };
        println!("Vertex buffer created successfully");
        let inner_size = window.inner_size();
        println!("Window size: {}x{}", inner_size.width, inner_size.height);
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

        let render_pass = get_render_pass(device.clone(), &swapchain);
        let framebuffers = get_framebuffers(&backbuffers, &render_pass);

        let command_buffer_allocator =
            StandardCommandBufferAllocator::new(device.clone(), Default::default());

        let effect = match CEEffectBuilder::new(CEEffectType::Rendering)
            .with_device(device.clone())
            .with_vertex_shader("line.vert".into(), "main".into())
            .with_pixel_shader("line.frag".into(), "main".into())
            .with_viewport(Arc::new(viewport))
            .with_render_pass(render_pass)
            .build::<MyVertex>()
        {
            Ok(e) => e,
            Err(e) => panic!("failed to create effect: {e:?}"),
        };

        let command_buffers = get_command_buffers(
            Arc::new(command_buffer_allocator),
            &queue,
            &effect.pipeline,
            &framebuffers,
            &buffer.buffer,
        );
        let fences: Vec<Option<Arc<Fence>>> = vec![None; command_buffers.len()];

        ChaosRenderSystem {
            physical_device: Some(physical_device),
            device: Some(device),
            queue: Some(queue),
            current_frame: 0,
            current_buffer: 0,
            framebuffers,
            command_buffers,
            swapchain,
            fences,
        }
    }

    pub fn render(&mut self) {
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
            .then_execute(
                self.queue.clone().unwrap(),
                self.command_buffers[image_i as usize].clone(),
            )
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
