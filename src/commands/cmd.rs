use gfx_hal::Backend;
use rendering::render_context::RenderContext;

pub trait Cmd {
    fn execute(&self);
    fn revert(&self) {
        // doesn't have to be implemented, but good to have
    }
}

pub trait RenderCmd<B: Backend> {
    fn render(&mut self, render_context: &mut RenderContext<B>);
}
