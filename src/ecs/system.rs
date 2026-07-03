use std::any::Any;

use crate::ecs::world::ChaosWorld;

pub trait ChaosSystem: Any {
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str>;

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str>;
}
