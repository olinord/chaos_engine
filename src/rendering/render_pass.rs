use std::mem::ManuallyDrop;

use gfx_hal::{Backend, command};
use gfx_hal::command::CommandBuffer;
use gfx_hal::pso::Rect;

use rendering::buffer::Buffer;
use rendering::core_renderer_utilities::create_render_pass;
use rendering::effect::Effect;

pub struct RenderPass<'a, B: Backend> {
    buffer: &'a mut Buffer<'a, B>,
    effect: &'a Effect<B>,
    render_pass: ManuallyDrop<B::RenderPass>,
}


impl<'a, B: Backend> RenderPass<'a, B> {
    pub fn new(device: &B::Device, effect: &'a Effect<B>, buffer: &'a mut Buffer<'a, B>) -> RenderPass<'a, B> {
        let render_pass = create_render_pass::<B>(&device);
        return RenderPass {
            buffer,
            effect,
            render_pass,
        }
    }

    pub fn render(&mut self, cmd_buffer: &mut B::CommandBuffer, framebuffer: &B::Framebuffer, render_area: Rect) {
        unsafe {
            self.effect.bind_to_cmd_buffer(cmd_buffer);
            self.buffer.bind_to_cmd_buffer(cmd_buffer);

            cmd_buffer.begin_render_pass(
                &self.render_pass,
                &framebuffer,
                render_area,
                &[], // this should be done indipendantly by the render_state
                command::SubpassContents::Inline,
            );
            cmd_buffer.draw(0..6, 0..1);
            cmd_buffer.end_render_pass();
        }
    }
}
