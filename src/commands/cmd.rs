use gfx_hal::Backend;
use rendering::render_context::RenderContext;

pub trait RenderCmd<B: Backend> {
    fn render(&mut self, render_context: &mut RenderContext<B>);
}

// Common traits
pub struct ExitCmd {}
