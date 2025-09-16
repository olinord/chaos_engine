use std::any::{Any, TypeId};

use crate::ecs::manager::ChaosComponentManager;

pub trait ChaosSystem: Any {
    fn initialize(&mut self, component_manager: &mut ChaosComponentManager);
    fn update(
        &mut self,
        delta_time: f32,
        component_manager: &mut ChaosComponentManager,
    ) -> Result<(), &'static str>;

    fn get_dependencies(&self) -> Vec<TypeId> {
        Vec::new()
    }
}
