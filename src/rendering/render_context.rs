use gfx_hal::{Backend, VertexCount};
use rendering::effect::Effect;
use gfx_hal::pass::Subpass;
use rendering::buffer::{Buffer, BufferData};
use gfx_hal::pso::{Viewport, Rect};
use gfx_hal::command::{CommandBuffer, CommandBufferFlags, ClearValue, ClearColor, SubpassContents};
use std::ops::Range;

pub struct RenderContext<'a, B: Backend> {
    device: &'a mut B::Device,
    main_pass: &'a mut B::RenderPass,
    physical_device: &'a mut B::PhysicalDevice,
    cmd_buffer: &'a mut B::CommandBuffer,
    frame_buffer: &'a mut B::Framebuffer,
}

impl<'a, B: Backend> RenderContext<'a, B> {
    pub fn new(device: &'a mut B::Device, main_pass: &'a mut B::RenderPass, physical_device: &'a mut B::PhysicalDevice, cmd_buffer: &'a mut B::CommandBuffer, frame_buffer: &'a mut B::Framebuffer) -> RenderContext<'a, B> {
        return RenderContext {
            device,
            main_pass,
            physical_device,
            cmd_buffer,
            frame_buffer
        }
    }

    pub fn prepare_effect(&mut self, effect: &mut Effect<B>) {
        if !effect.is_initialized() {
            let subpass = Subpass {
                index: 0,
                main_pass: &*self.main_pass,
            };
            if let Err(res) =  effect.initialize(self.device, &subpass) {
                println!("Error initializing effect {}", res);
            }
        }
        effect.bind_to_cmd_buffer(self.cmd_buffer);
    }

    pub fn prepare_buffer<T: BufferData>(&mut self, buffer: &mut Buffer<B, T>) {
        if !buffer.is_initialized() {
            buffer.initialize(self.device, self.physical_device);
        }
        buffer.bind_to_cmd_buffer(self.cmd_buffer);
    }

    pub fn begin(&mut self, clear_color: [f32; 4], viewport: &Viewport, scissor: Option<Rect>, render_area: Option<Rect>) {
        unsafe {
            self.cmd_buffer.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

            self.cmd_buffer.set_viewports(0, &[viewport.clone()]);
            self.cmd_buffer.set_scissors(0, &[scissor.or(Some(viewport.rect)).unwrap()]);

            self.cmd_buffer.begin_render_pass(
                &self.main_pass,
                &self.frame_buffer,
                render_area.or(Some(viewport.rect)).unwrap(),
                &[ClearValue {
                    color: ClearColor {
                        float32: clear_color,
                    },
                }],
                SubpassContents::Inline,
            );
        }
    }

    pub fn draw(&mut self, vertex_count: Range<VertexCount>, instance_count: Range<VertexCount>) {
        unsafe {
            self.cmd_buffer.draw(vertex_count, instance_count);
        }
    }

    pub fn end(&mut self) {
        unsafe {
            self.cmd_buffer.end_render_pass();
            self.cmd_buffer.finish();
        }
    }

}