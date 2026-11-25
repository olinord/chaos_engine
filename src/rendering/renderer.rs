use ash::vk;
use log::debug;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use vulkano::{
    VulkanError, VulkanLibrary, VulkanObject,
    command_buffer::allocator::StandardCommandBufferAllocator,
    device::{Device, DeviceCreateInfo, DeviceFeatures, Queue, QueueCreateInfo, QueueFlags},
    image::ImageLayout,
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::StandardMemoryAllocator,
    swapchain::{
        AcquireNextImageInfo, PresentInfo, SemaphorePresentInfo, Surface, SwapchainPresentInfo,
    },
    sync::{
        fence::{Fence, FenceCreateFlags, FenceCreateInfo},
        semaphore::{Semaphore, SemaphoreCreateInfo},
    },
};
use winit::{raw_window_handle::DisplayHandle, window::Window};

use crate::{
    engine_const::FRAMES_IN_FLIGHT,
    frame_zone,
    rendering::{
        adapters::select_physical_device,
        command_buffer::{
            ChaosClearValue, ChaosCommandBuffer, ChaosRenderAttachmentLoadOp, FinishedFrame,
        },
        context::ChaosRenderContext,
        draw_command::ChaosDrawQueue,
        errors::{RenderError, RenderResultExt},
        graph::{
            graph::{Graph, GraphContext, GraphError},
            node::{GraphNode, GraphNodeType},
            resource::{ImportedResourceSetting, ResourceDescription, ResourceUsage},
        },
        swapchain::get_swapchain_and_backbuffers,
    },
};

/// FrameSlot holds the resources for a single frame in flight, including the finished frame, fence, and image available semaphore.
/// It is stored in a FRAMES_IN_FLIGHT-sized array in ChaosRenderer, and is indexed by render_context.frame_index() % FRAMES_IN_FLIGHT.
/// Before a frame is started we wait for the fence to signal and clear the finished frame. The fence is reset right
/// before the submit, and after the submit the finished frame is stored in the slot until the fence signals again.
///
/// Deliberately not `Clone`: cloning would share the same fence/semaphore between slots.
pub struct FrameSlot {
    pub fence: Arc<Fence>,
    pub image_available: Arc<Semaphore>,
    pub finished_frame: Option<FinishedFrame>,
}

impl FrameSlot {
    fn new(device: Arc<Device>) -> Result<Self, RenderError> {
        // create a fence that is initially signaled, so the first frame can start without waiting
        // create a semaphore for the image available signal
        // create a finished frame that is None, since we haven't submitted anything yet
        Ok(Self {
            fence: Arc::new(
                Fence::new(
                    device.clone(),
                    FenceCreateInfo {
                        flags: FenceCreateFlags::SIGNALED,
                        ..Default::default()
                    },
                )
                .context("creating a frame slot fence")?,
            ),
            image_available: Arc::new(
                Semaphore::new(device.clone(), SemaphoreCreateInfo::default())
                    .context("creating a frame slot image available semaphore")?,
            ),
            finished_frame: None,
        })
    }
}

struct QueueRendererNodeType {
    pub draw_queue: Arc<Mutex<ChaosDrawQueue>>,
}

impl GraphNodeType for QueueRendererNodeType {
    fn execute(&self, mut ctx: GraphContext) -> Result<(), GraphError> {
        let mut draw_queue = std::mem::take(&mut *self.draw_queue.lock().unwrap());
        draw_queue
            .sort_by_key(|draw_command| (draw_command.render_phase, draw_command.effect_hash));

        // sort the draw queue by sort key, then execute each draw function in order
        for draw_command in draw_queue {
            frame_zone!("render draw command");
            match (draw_command.draw_function)(ctx.command_buffer) {
                Ok(()) => {}
                Err(e) if e.recoverable() => ctx.report(format!("Draw command failed {e}")),
                Err(e) => {
                    return Err(GraphError::Render(e));
                }
            }
        }

        Ok(())
    }
}

/// ChaosRenderer is the main rendering system. It manages the swapchain, command buffers, and frames in flight.
/// It is responsible for starting and ending frames, submitting command buffers, and handling swapchain recreation.
#[allow(unused)]
pub struct ChaosRenderer {
    render_context: Arc<ChaosRenderContext>,
    queue: Arc<Queue>,
    current_image_index: u32, // the swapchain image index
    frames: [FrameSlot; FRAMES_IN_FLIGHT],
    render_finished: Vec<Arc<Semaphore>>, // Semaphores to signal when rendering is finished for each frame, indexed by current_image_index
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    directories: HashMap<PathBuf, PathBuf>,
    pending_resize: Option<[u32; 2]>,
    graph: Graph,
    draw_queue: Arc<Mutex<ChaosDrawQueue>>,
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

        let mut render_finished = Vec::new();
        for _ in backbuffers.iter() {
            // create a semaphore for each backbuffer to signal when rendering is finished
            render_finished.push(Arc::new(
                Semaphore::new(device.clone(), SemaphoreCreateInfo::default())
                    .context("creating a render finished semaphore")?,
            ));
        }

        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
            device.clone(),
            Default::default(),
        ));

        let render_context = Arc::new(ChaosRenderContext::new(
            physical_device.clone(),
            device.clone(),
            memory_allocator.clone(),
            swapchain,
            backbuffers,
            graphic_family_index,
            0.0,
        ));

        // Each slot needs its own fence and semaphore, so create them separately.
        let frames = [
            FrameSlot::new(device.clone())?,
            FrameSlot::new(device.clone())?,
            FrameSlot::new(device)?,
        ];

        let draw_queue: Arc<Mutex<ChaosDrawQueue>> = Arc::new(Mutex::new(ChaosDrawQueue::new()));
        let node = GraphNode::new(
            "Main".to_string(),
            QueueRendererNodeType {
                draw_queue: draw_queue.clone(),
            },
        )
        .with_resource(
            "backbuffer".to_string(),
            ResourceDescription::Imported {
                resource_name: "backbuffer".to_string(),
                setting: ImportedResourceSetting {
                    initial_layout: ImageLayout::Undefined,
                    end_layout: ImageLayout::PresentSrc,
                },
            },
            ResourceUsage::ColorAttachment(ChaosRenderAttachmentLoadOp::Clear(
                ChaosClearValue::Float([0.0, 0.0, 0.0, 1.0]),
            )),
            true,
        );
        let mut graph = Graph::new();
        graph.set_display_size([inner_size.width, inner_size.height]);
        let _ = graph.add_node(node).context("Creating the graph")?;

        Ok(ChaosRenderer {
            render_context: render_context.clone(),
            queue,
            current_image_index: 0,
            frames,
            render_finished,
            command_buffer_allocator,
            directories: directories.clone(),
            pending_resize: None,
            graph,
            draw_queue,
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

        let frame = &mut self.frames[self.render_context.frame_index() % FRAMES_IN_FLIGHT];
        frame
            .fence
            .wait(None)
            .context("wait for last finished frame to complete")?;
        frame.finished_frame = None; // drop the previous finished frame so its resources can be freed

        self.current_image_index = unsafe {
            // aquire the next image
            let acquire_info = AcquireNextImageInfo {
                timeout: None,
                semaphore: Some(frame.image_available.clone()),
                fence: None,
                ..Default::default()
            };
            match self
                .render_context
                .swapchain()
                .acquire_next_image(&acquire_info)
                .context("acquire next swapchain image")
            {
                Ok(acquired_info) => {
                    if acquired_info.is_suboptimal {
                        // Still usable this frame, but recreate before the next one.
                        self.pending_resize = Some(self.render_context.swapchain().image_extent());
                    }
                    acquired_info.image_index
                }
                Err(RenderError::SwapchainOutOfDate) => {
                    self.pending_resize = Some(self.render_context.swapchain().image_extent());
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        };

        // clear the last frame's descriptor set cache
        self.render_context.clear_descriptor_set_cache();

        let command_buffer = ChaosCommandBuffer::new(
            self.command_buffer_allocator.clone(),
            self.queue.queue_family_index(),
        )?;
        Ok(Some(command_buffer))
    }

    /// Records every draw command. A failing draw command (e.g. an effect whose buffers were
    /// never uploaded) is skipped and logged once, instead of taking the whole frame down.
    pub fn render(
        &mut self,
        draw_queue: ChaosDrawQueue,
        command_buffer: &mut ChaosCommandBuffer,
    ) -> Result<(), RenderError> {
        let backbuffers = self.render_context.backbuffers();
        let backbuffer = backbuffers[self.current_image_index as usize].clone();

        let destinations = HashMap::from([("backbuffer".to_string(), backbuffer)]);

        *self.draw_queue.lock().unwrap() = draw_queue;

        self.graph
            .render(&destinations, command_buffer, &self.render_context)
            .context("rendering the graph")
    }

    pub fn end_frame(&mut self, command_buffer: ChaosCommandBuffer) -> Result<(), RenderError> {
        let slot_index = self.render_context.frame_index() % FRAMES_IN_FLIGHT;
        let image_index = self.current_image_index as usize;

        let finished = command_buffer.finish()?;

        // The ash structs hold *pointers* to these arrays, so they must be named
        // locals that outlive the submit call. Temporaries won't work.
        let wait_semaphores = [self.frames[slot_index].image_available.as_ref().handle()];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [finished.cmd_buffer.handle()];
        let signal_semaphores = [self.render_finished[image_index].as_ref().handle()];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages) // one stage per wait semaphore
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores);

        let fence = self.frames[slot_index].fence.clone();
        unsafe { fence.reset().context("resetting frame slot fence") }?; // reset right before the submit (see earlier)

        self.queue.with(|_guard| unsafe {
            // `_guard` holds the queue's lock: Vulkan requires queue access to be externally
            // synchronized, and vulkano uses this lock for its own submits/presents.
            let fns = self.queue.device().fns();
            (fns.v1_0.queue_submit)(
                self.queue.as_ref().handle(),
                1,
                &submit_info,
                fence.handle(),
            )
            .result()
            .map_err(VulkanError::from)
            .context("render queue submit")
        })?;

        // The fence signals when the GPU is done with this submit. Until then the command
        // buffer and every keep-alive must stay alive, so they live in the slot;
        // start_frame drops them after waiting on this slot's fence.
        self.frames[slot_index].finished_frame = Some(finished);

        // Hand the image back to the window once the submit above has signalled
        // this image's render_finished semaphore.
        let present_info = PresentInfo {
            wait_semaphores: vec![SemaphorePresentInfo::new(
                self.render_finished[image_index].clone(),
            )],
            swapchain_infos: vec![SwapchainPresentInfo::swapchain_image_index(
                self.render_context.swapchain(),
                self.current_image_index,
            )],
            ..Default::default()
        };
        let present_result = self
            .queue
            .with(|mut q| unsafe {
                // One result per swapchain; collect inside the closure so nothing
                // borrows the queue guard.
                q.present(&present_info)
                    .map(|results| results.collect::<Vec<_>>())
            })
            .context("present swapchain image")
            .and_then(|results| {
                results
                    .into_iter()
                    .next()
                    .unwrap_or(Ok(false))
                    .context("present swapchain image")
            });
        self.render_context.increment_frame_index();

        match present_result {
            Ok(suboptimal) => {
                if suboptimal {
                    self.pending_resize = Some(self.render_context.swapchain().image_extent());
                }
            }
            // The window changed; not fatal, recreate before the next frame.
            Err(RenderError::SwapchainOutOfDate) => {
                self.pending_resize = Some(self.render_context.swapchain().image_extent());
            }
            Err(e) => return Err(e),
        }

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
        // Wait for the whole queue, not just the frame fences: presents aren't covered by a fence
        // and still wait on the render_finished semaphores we're about to destroy.
        self.queue
            .with(|mut q| q.wait_idle())
            .context("wait for the queue to go idle before recreating the swapchain")?;

        for frame in &mut self.frames {
            frame.finished_frame = None; // drop the finished frame so its resources can be freed
        }

        // recreate the swapchain and backbuffers
        self.render_context.recreate_swapchain(extent)?;

        // recreate the render finished semaphores for each backbuffer
        self.render_finished.clear();
        for _ in self.render_context.backbuffers().iter() {
            // create a semaphore for each backbuffer to signal when rendering is finished
            self.render_finished.push(Arc::new(
                Semaphore::new(
                    self.render_context.device().clone(),
                    SemaphoreCreateInfo::default(),
                )
                .context("creating a render finished semaphore")?,
            ));
        }

        self.graph.set_display_size(extent);
        // no upscaling is supported rn
        self.graph.set_render_size(extent);
        Ok(())
    }
}
