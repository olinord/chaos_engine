use log::debug;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use vulkano::{
    VulkanLibrary,
    command_buffer::allocator::StandardCommandBufferAllocator,
    device::{Device, DeviceCreateInfo, DeviceFeatures, Queue, QueueCreateInfo, QueueFlags},
    image::ImageLayout,
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::StandardMemoryAllocator,
    pipeline::graphics::viewport::Scissor,
    swapchain::{self, PresentFuture, Surface, SwapchainAcquireFuture, SwapchainPresentInfo},
    sync::{
        self, GpuFuture,
        future::{FenceSignalFuture, JoinFuture},
    },
};
use winit::{raw_window_handle::DisplayHandle, window::Window};

use crate::rendering::{
    adapters::select_physical_device,
    command_buffer::{
        ChaosClearValue, ChaosCommandBuffer, ChaosRenderAttachment, ChaosRenderAttachmentLoadOp,
        ChaosRenderExtent,
    },
    context::ChaosRenderContext,
    draw_command::ChaosDrawQueue,
    errors::{RenderError, RenderResultExt},
    swapchain::get_swapchain_and_backbuffers,
    texture::ChaosTexture,
};

use crate::frame_zone;

// TODO(step 5): this future chain no longer executes the frame's command buffer
// (raw command buffers can't go through `then_execute`); replace with explicit
// semaphores/fences + queue submit/present.
pub type Fence =
    FenceSignalFuture<PresentFuture<JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture>>>;

#[allow(unused)]
pub struct ChaosRenderer {
    render_context: Arc<ChaosRenderContext>,
    queue: Arc<Queue>,
    current_frame: u128,
    current_buffer: u32,
    current_acquire_future: Option<SwapchainAcquireFuture>,
    fences: Vec<Option<Fence>>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    directories: HashMap<PathBuf, PathBuf>,
    pending_resize: Option<[u32; 2]>,
    // Draw command failures that were already logged, so a broken draw doesn't log every frame.
    reported_draw_errors: HashSet<&'static str>,
}

impl ChaosRenderer {
    pub fn new(
        display_handle: &DisplayHandle,
        window: Arc<Window>,
        directories: &HashMap<PathBuf, PathBuf>,
    ) -> Result<ChaosRenderer, RenderError> {
        let library = VulkanLibrary::new()
            .map_err(|e| RenderError::initialization("load the Vulkan library", e))?;

        let required_extensions = Surface::required_extensions(display_handle)
            .map_err(|e| RenderError::initialization("query required surface extensions", e))?;

        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: required_extensions,
                ..Default::default()
            },
        )
        .map_err(|e| RenderError::initialization("create Vulkan instance", e))?;

        let surface = Surface::from_window(instance.clone(), window.clone())
            .map_err(|e| RenderError::initialization("create window surface", e))?;

        let device_extensions = vulkano::device::DeviceExtensions {
            khr_swapchain: true,
            khr_dynamic_rendering: true,
            ..vulkano::device::DeviceExtensions::empty()
        };
        let (physical_device, queue_family_index) =
            select_physical_device(&instance, &surface, &device_extensions)?;

        let graphic_family_index: u32 = physical_device
            .queue_family_properties()
            .iter()
            .enumerate()
            .find(|(_, qf)| qf.queue_flags.contains(QueueFlags::GRAPHICS))
            .map(|(index, _)| index as u32)
            .expect("select_physical_device only returns devices with a graphics queue");

        let (device, mut queues) = Device::new(
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
                    multi_draw_indirect: true,
                    draw_indirect_first_instance: true,
                    shader_draw_parameters: true,
                    descriptor_indexing: true,
                    runtime_descriptor_array: true,
                    shader_sampled_image_array_non_uniform_indexing: true,
                    descriptor_binding_partially_bound: true,
                    descriptor_binding_update_unused_while_pending: true,
                    descriptor_binding_variable_descriptor_count: true,
                    ..DeviceFeatures::empty()
                },
                ..Default::default()
            },
        )
        .map_err(|e| RenderError::initialization("create logical device", e))?;

        debug!("Device created successfully");
        let queue = queues
            .next()
            .expect("Device::new returns one queue per QueueCreateInfo");

        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

        let inner_size = window.inner_size();
        let (swapchain, backbuffers) = get_swapchain_and_backbuffers(
            physical_device.clone(),
            device.clone(),
            surface.clone(),
            [inner_size.width, inner_size.height],
        )?;

        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
            device.clone(),
            Default::default(),
        ));

        let fences: Vec<Option<Fence>> = (0..3).map(|_| None).collect();

        let render_context = Arc::new(ChaosRenderContext::new(
            physical_device.clone(),
            device.clone(),
            memory_allocator.clone(),
            swapchain,
            backbuffers,
            graphic_family_index,
            [inner_size.width as f32, inner_size.height as f32],
            0.0,
        ));

        Ok(ChaosRenderer {
            render_context: render_context.clone(),
            queue,
            current_frame: 0,
            current_buffer: 0,
            current_acquire_future: None,
            fences,
            command_buffer_allocator,
            directories: directories.clone(),
            pending_resize: None,
            reported_draw_errors: HashSet::new(),
        })
    }

    /// Acquires the next swapchain image and starts recording the frame.
    ///
    /// Returns `Ok(None)` when the frame should be skipped because the swapchain is out of
    /// date; it is recreated before the next frame. Any `Err` is fatal.
    pub fn start_frame(&mut self) -> Result<Option<ChaosCommandBuffer>, RenderError> {
        if let Some(extent) = self.pending_resize.take() {
            match self.recreate_swapchain_now(extent) {
                Ok(()) => {}
                // The surface changed again while recreating; try again next frame.
                Err(RenderError::SwapchainOutOfDate) => {
                    self.pending_resize = Some(extent);
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        }

        let (image_i, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.render_context.swapchain(), None)
                .context("acquire next swapchain image")
            {
                Ok(r) => r,
                Err(RenderError::SwapchainOutOfDate) => {
                    self.pending_resize = Some(self.render_context.swapchain().image_extent());
                    return Ok(None);
                }
                Err(e) => return Err(e),
            };

        if suboptimal {
            // Render this frame anyway, but recreate the swapchain before the next one.
            self.pending_resize = Some(self.render_context.swapchain().image_extent());
        }

        if let Some(image_fence) = &self.fences[image_i as usize] {
            image_fence
                .wait(None)
                .context("wait for previous frame on this image")?;
        }

        // clear the last frame's descriptor set cache
        self.render_context.clear_descriptor_set_cache();

        self.current_buffer = image_i;
        self.current_acquire_future = Some(acquire_future);

        let backbuffers = self.render_context.backbuffers();

        self.begin_rendering(backbuffers[image_i as usize].clone())
            .map(Some)
    }

    pub fn begin_rendering(
        &self,
        destination: Arc<ChaosTexture>,
    ) -> Result<ChaosCommandBuffer, RenderError> {
        let mut command_buffer = ChaosCommandBuffer::new(
            self.command_buffer_allocator.clone(),
            self.queue.queue_family_index(),
        )?;

        let color_attachment = Arc::new(ChaosRenderAttachment::new(
            destination,
            ChaosRenderAttachmentLoadOp::Clear(ChaosClearValue::Float([0.0, 0.0, 0.0, 1.0])),
            ImageLayout::ColorAttachmentOptimal,
            true,
        ));

        let viewport_extent = self.render_context.viewport().extent;
        // Framebuffer-space extent is always positive; the viewport uses a
        // negative height to flip Y (see `flipped_viewport`), which must not
        // leak into render-area or scissor sizes.
        let fb_extent = [
            viewport_extent[0].abs() as u32,
            viewport_extent[1].abs() as u32,
        ];
        // Push the current viewport/scissor to the command buffer so pipelines
        // built with dynamic viewport state (see `EffectFactory`) pick up the
        // current window size after a resize.
        command_buffer
            .begin_rendering(
                &[Some(color_attachment)],
                None,
                None,
                Some(ChaosRenderExtent {
                    size: fb_extent,
                    offset: None,
                }),
            )
            .set_viewport(self.render_context.viewport())
            .set_scissor(Scissor {
                offset: [0, 0],
                extent: fb_extent,
            });
        Ok(command_buffer)
    }

    /// Records every draw command. A failing draw command (e.g. an effect whose buffers were
    /// never uploaded) is skipped and logged once, instead of taking the whole frame down.
    pub fn render(&mut self, draw_queue: ChaosDrawQueue, command_buffer: &mut ChaosCommandBuffer) {
        // sort the draw queue by sort key, then execute each draw function in order
        let mut draw_queue = draw_queue;
        draw_queue
            .sort_by_key(|draw_command| (draw_command.render_phase, draw_command.effect_hash));
        for draw_command in draw_queue {
            frame_zone!("render draw command");
            if let Err(error) = (draw_command.draw_function)(command_buffer) {
                if self.reported_draw_errors.insert(error) {
                    log::error!("Skipping draw command: {error}");
                }
            }
        }
    }

    pub fn end_frame(&mut self, mut command_buffer: ChaosCommandBuffer) -> Result<(), RenderError> {
        command_buffer.end_rendering();
        // TODO(step 5): submit `finished_frame.cmd_buffer` and keep `finished_frame`
        // alive until this frame's fence signals. Until then the recorded work is
        // dropped here and never reaches the GPU.
        let _finished_frame = command_buffer.finish()?;

        let image_i = self.current_buffer;
        let acquire_future = self
            .current_acquire_future
            .take()
            .expect("end_frame called before start_frame acquired a swapchain image");

        let previous_future = match self.fences[image_i as usize].take() {
            // Create a NowFuture
            None => {
                let mut now = sync::now(self.render_context.device());
                now.cleanup_finished();

                now.boxed()
            }
            // Use the existing FenceSignalFuture
            Some(fence) => fence.boxed(),
        };

        let future = previous_future
            .join(acquire_future)
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(
                    self.render_context.swapchain(),
                    image_i,
                ),
            )
            .then_signal_fence_and_flush();

        self.fences[image_i as usize] = match future.context("present frame") {
            Ok(value) => Some(value),
            Err(RenderError::SwapchainOutOfDate) => {
                self.pending_resize = Some(self.render_context.swapchain().image_extent());
                None
            }
            Err(e) => return Err(e),
        };
        self.current_buffer = image_i;
        self.current_frame = self.current_frame.wrapping_add(1);
        Ok(())
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

    fn recreate_swapchain_now(&mut self, extent: [u32; 2]) -> Result<(), RenderError> {
        // Wait for any in-flight frames before retiring the current swapchain.
        for fence_slot in self.fences.iter_mut() {
            if let Some(fence) = fence_slot.take() {
                fence
                    .wait(None)
                    .context("wait for in-flight frames before swapchain recreation")?;
            }
        }
        self.render_context.recreate_swapchain(extent)
    }
}
