use std::any::Any;
use rendering::render_state::RenderState;
use gfx_hal::Backend;

pub trait Component: Any {}
impl<T: Any> Component for T {}

pub trait RenderComponent<T: Backend>: Any{
    fn initialize(&mut self, render_state: &mut RenderState<T>) -> Result<(), &'static str>;
    fn needs_initializing(&self) -> bool;
}
