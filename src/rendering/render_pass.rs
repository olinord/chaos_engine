use std::mem::ManuallyDrop;

use gfx_hal::{
    Backend,
    command::CommandBuffer
};

use rendering::buffer::Buffer;
use rendering::core_renderer_utilities::create_render_pass;
use rendering::effect::Effect;

pub struct RenderPass<B: Backend> {
    buffer: Buffer<B>,
    effect: Effect<B>,
}


impl<B: Backend> RenderPass<B> {
    pub fn new(effect: Effect<B>, buffer: Buffer<B>) -> RenderPass<B> {
        return RenderPass {
            buffer,
            effect,
        };
    }

    pub fn bind_buffer(&mut self, cmd_buffer: &mut B::CommandBuffer) {
        self.buffer.bind_to_cmd_buffer(cmd_buffer);
    }

    pub fn render(&mut self, cmd_buffer: &mut B::CommandBuffer) {
        unsafe {
            self.effect.bind_to_cmd_buffer(cmd_buffer);
            cmd_buffer.draw(0..self.buffer.get_length(), 0..1);
        }
    }
}
