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
    // render_pass: ManuallyDrop<B::RenderPass>,
}


impl<B: Backend> RenderPass<B> {
    pub fn new(device: &B::Device, effect: Effect<B>, buffer: Buffer<B>) -> RenderPass<B> {
        // let render_pass = create_render_pass::<B>(&device);
        return RenderPass {
            buffer,
            effect,
            // render_pass,
        };
    }

    pub fn bind_buffer(&mut self, cmd_buffer: &mut B::CommandBuffer) {
        self.buffer.bind_to_cmd_buffer(cmd_buffer);
    }

    pub fn render(&mut self, cmd_buffer: &mut B::CommandBuffer) {
        unsafe {
            self.effect.bind_to_cmd_buffer(cmd_buffer);
            //
            // cmd_buffer.begin_render_pass(
            //     &self.render_pass,
            //     &framebuffer,
            //     render_area,
            //     &[], // this should be done independently by the render_state
            //     command::SubpassContents::Inline,
            // );
            cmd_buffer.draw(0..self.buffer.get_length(), 0..1);

            // cmd_buffer.end_render_pass();
        }
    }
}
