use std::any::Any;

use ecs::manager::ChaosComponentManager;
use rendering::render_state::RenderState;

pub trait ChaosService: Any {
    fn initialize(&mut self);
    fn update(&mut self, delta_time: f32, component_manager: &mut ChaosComponentManager);
}


pub trait ChaosRenderService<'a>: Any {
    fn initialize(&mut self, render_state: &mut RenderState<'a, back::Backend>);
    fn update(&mut self, delta_time: f32, component_manager: &mut ChaosComponentManager, render_state: &mut RenderState<'a, back::Backend>);
}