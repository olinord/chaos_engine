use std::any::Any;

use crate::ecs::component::ChaosComponentManager;

pub trait ChaosSystem: Any {
    fn initialize(&mut self);
    fn update(
        &mut self,
        delta_time: f32,
        component_manager: &mut ChaosComponentManager,
    ) -> Result<(), &'static str>;
}
