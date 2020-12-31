use core::{iter, ptr};
use std::borrow::Borrow;
use std::mem::ManuallyDrop;

use gfx_hal::{
    adapter::Adapter,
    Backend,
    device::Device,
    image::Extent,
    Instance,
    pool::CommandPool,
    pso::{Rect, Viewport},
    queue::{CommandQueue, QueueGroup, Submission},
    window::{Extent2D, PresentationSurface},
};
use winit::Window;

use engine_const::{ENGINE_NAME, ENGINE_VERSION};
use rendering::core_renderer_utilities::create_render_pass;

use super::core_renderer_utilities::{create_command_buffer, create_command_pool, create_swapchain, extract_adapter, extract_device_and_queue_group};
use commands::cmd::RenderCmd;
use rendering::render_context::RenderContext;
use std::slice::IterMut;

pub struct RenderState<B: Backend> {
    #[allow(unused)]
    window: Window,
    instance: Option<B::Instance>,
    device: B::Device,
    queue_group: QueueGroup<B>,
    surface: ManuallyDrop<B::Surface>,
    adapter: Adapter<B>,
    dimensions: Extent2D,
    viewport: Viewport,
    main_pass: ManuallyDrop<B::RenderPass>,
    submission_complete_semaphores: Vec<B::Semaphore>,
    submission_complete_fences: Vec<B::Fence>,
    cmd_pools: Vec<B::CommandPool>,
    cmd_buffers: Vec<B::CommandBuffer>,
    frames_in_flight: usize,
    frame_index: usize,
}

impl<B: Backend> RenderState<B> {
    pub fn new(window: Window) -> Result<RenderState<B>, &'static str> {
        log::info!("Initializing RenderState");

        // Backend specific black box
        let instance = B::Instance::create(ENGINE_NAME, ENGINE_VERSION).unwrap();

        // link between the window and the instance (to draw on)
        let mut surface: B::Surface = unsafe {
            instance.create_surface(&window).unwrap()
        };

        // ... the adapter...
        let adapter = extract_adapter::<B>(&instance, &surface);

        // the logical device and the queue group for the command buffers
        let (device, queue_group) = extract_device_and_queue_group::<B>(&adapter, &surface).unwrap();

        // Physical size is the real-life size of the display, in physical pixels.
        // Logical size is the scaled display, according to the OS. High-DPI
        // displays will present a smaller logical size, which you can scale up by
        // the DPI to determine the physical size.
        let physical_window_size: (u32, u32) = window.get_inner_size().unwrap().to_physical(window.get_hidpi_factor()).into();
        let dimensions = Extent2D { width: physical_window_size.0, height: physical_window_size.1 };
        create_swapchain(dimensions, &adapter, &mut surface, &device);

        let frames_in_flight: usize = 3;
        let frame_index: usize = 0;

        // The number of the rest of the resources is based on the frames in flight.
        let mut submission_complete_semaphores = Vec::with_capacity(frames_in_flight);
        let mut submission_complete_fences = Vec::with_capacity(frames_in_flight);
        // Note: We don't really need a different command pool per frame in such a simple demo like this,
        // but in a more 'real' application, it's generally seen as optimal to have one command pool per
        // thread per frame. There is a flag that lets a command pool reset individual command buffers
        // which are created from it, but by default the whole pool (and therefore all buffers in it)
        // must be reset at once. Furthermore, it is often the case that resetting a whole pool is actually
        // faster and more efficient for the hardware than resetting individual command buffers, so it's
        // usually best to just make a command pool for each set of buffers which need to be reset at the
        // same time (each frame). In our case, each pool will only have one command buffer created from it,
        // though.
        let mut cmd_pools: Vec<B::CommandPool> = Vec::with_capacity(frames_in_flight);
        let mut cmd_buffers: Vec<B::CommandBuffer> = Vec::with_capacity(frames_in_flight);

        for index in 0..frames_in_flight {
            cmd_pools.push(create_command_pool::<B>(&device, queue_group.family));
            cmd_buffers.push(create_command_buffer::<B>(cmd_pools.get_mut(index).unwrap()));

            submission_complete_semaphores.push(
                device
                    .create_semaphore()
                    .expect("Could not create semaphore"),
            );
            submission_complete_fences
                .push(device.create_fence(true).expect("Could not create fence"));
        }

        let physical_window_size = &window.get_inner_size().unwrap().to_physical(window.get_hidpi_factor());

        // Rendering setup
        let viewport = Viewport {
            rect: Rect {
                x: 0,
                y: 0,
                w: physical_window_size.width as _,
                h: physical_window_size.height as _,
            },
            depth: 0.0..1.0,
        };

        let main_pass = create_render_pass::<B>(&device);

        return Ok(RenderState {
            window,
            instance: Some(instance),
            surface: ManuallyDrop::new(surface),
            adapter,
            frames_in_flight,
            frame_index,
            device,
            queue_group,
            cmd_pools,
            cmd_buffers,
            submission_complete_semaphores,
            submission_complete_fences,
            viewport,
            dimensions,
            main_pass,
        });
    }

    fn recreate_swapchain(&mut self) {
        create_swapchain(self.dimensions, &self.adapter, &mut self.surface, &self.device);
    }

    pub fn render(&mut self, render_commands: IterMut<'_, Box<dyn RenderCmd<B>>>) -> Result<(), &'static str> {
        let surface_image = unsafe {
            match self.surface.acquire_image(!0) {
                Ok((image, _)) => image,
                Err(_) => {
                    self.recreate_swapchain();
                    return Ok(());
                }
            }
        };

        let mut framebuffer = unsafe {
            self.device
                .create_framebuffer(
                    &self.main_pass,
                    iter::once(surface_image.borrow()),
                    Extent {
                        width: self.dimensions.width,
                        height: self.dimensions.height,
                        depth: 1,
                    },
                )
                .unwrap()
        };

        // Compute index into our resource ring buffers based on the frame number
        // and number of frames in flight. Pay close attention to where this index is needed
        // versus when the swapchain image index we got from acquire_image is needed.
        self.frame_index = self.frame_index % self.frames_in_flight;

        // Wait for the fence of the previous submission of this frame and reset it; ensures we are
        // submitting only up to maximum number of frames_in_flight if we are submitting faster than
        // the gpu can keep up with. This would also guarantee that any resources which need to be
        // updated with a CPU->GPU data copy are not in use by the GPU, so we can perform those updates.
        // In this case there are none to be done, however.
        unsafe {
            let fence = &self.submission_complete_fences[self.frame_index];
            self.device
                .wait_for_fence(fence, !0)
                .expect("Failed to wait for fence");
            self.device
                .reset_fence(fence)
                .expect("Failed to reset fence");
            self.cmd_pools[self.frame_index].reset(false);
        }

        let cmd_buffer = &mut self.cmd_buffers[self.frame_index];
        let mut render_context = RenderContext::new(&mut self.device, &mut *self.main_pass, &mut self.adapter.physical_device, cmd_buffer, &mut framebuffer);

        let clear_color = [0.05, 0.05, 0.05, 1.0];
        render_context.begin(clear_color, &self.viewport, None, None);

        for command in render_commands {
            command.render(&mut render_context);
        }

        render_context.end();

        unsafe {
            let submission = Submission {
                command_buffers: iter::once(&*cmd_buffer),
                wait_semaphores: None,
                signal_semaphores: iter::once(&self.submission_complete_semaphores[self.frame_index]),
            };

            self.queue_group.queues[0].submit(
                submission,
                Some(&self.submission_complete_fences[self.frame_index]),
            );

            // present frame
            let result = self.queue_group.queues[0].present(
                &mut self.surface,
                surface_image,
                Some(&self.submission_complete_semaphores[self.frame_index]),
            );

            self.device.destroy_framebuffer(framebuffer);

            if result.is_err() {
                self.recreate_swapchain();
            }
        }

        self.frame_index += 1;

        return Ok(());
    }

    // pub fn add_render_pass<T: BufferData>(&mut self, data_list: &[T], vs_path: String, ps_path: String) -> Result<(), &'static str> {
    //     let subpass = Subpass {
    //         index: 0,
    //         main_pass: &*self.main_pass,
    //     };
    //
    //     let effect = Effect::vertex_pixel::<T>(&self.device, vs_path, ps_path, &subpass)?;
    //
    //     let buffer = Buffer::vertex_buffer(data_list, &self.device, &self.adapter.physical_device);
    //     let render_pass = RenderPass::<B>::new(effect, buffer);
    //     self.render_passes.push(render_pass);
    //     Ok(())
    // }

    // pub fn create_buffer<T: BufferData>(&mut self, data_list: &[T]) -> Buffer<B> {
    //     Buffer::vertex_buffer(data_list, &self.device, &self.adapter.physical_device)
    // }

    // pub fn create_effect<T: BufferData>(&mut self, vs_path: String, ps_path: String) -> Result<Effect<B>, &'static str> {
    //     let subpass = Subpass {
    //         index: 0,
    //         main_pass: &*self.main_pass,
    //     };
    //     Effect::vertex_pixel::<T>(&self.device, vs_path, ps_path, &subpass)
    // }
}

impl<B: Backend> Drop for RenderState<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();
        unsafe {
            for p in self.cmd_pools.drain(..) {
                self.device.destroy_command_pool(p);
            }
            for s in self.submission_complete_semaphores.drain(..) {
                self.device.destroy_semaphore(s);
            }
            for f in self.submission_complete_fences.drain(..) {
                self.device.destroy_fence(f);
            }
            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(ptr::read(&self.main_pass)));
            self.surface.unconfigure_swapchain(&self.device);
            if let Some(instance) = &self.instance {
                let surface = ManuallyDrop::into_inner(ptr::read(&self.surface));
                instance.destroy_surface(surface);
            }
        }
        println!("DROPPED!");
    }
}
