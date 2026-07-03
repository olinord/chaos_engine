use crate::components::transform::TransformComponent;
use crate::components::velocity::VelocityComponent;
use chaos_engine::{ecs::system::ChaosSystem, ecs::world::ChaosWorld};
pub struct TransformSystem {}

impl TransformSystem {
    pub fn new() -> Self {
        Self {}
    }
}

impl ChaosSystem for TransformSystem {
    fn initialize(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        let delta_time = world.get_time().delta_time();
        let mut query = world
            .query::<(&mut TransformComponent, &VelocityComponent)>()
            .map_err(|_| "Failed to query transform components")?;

        for (_, (transform, velocity)) in query.iter_mut() {
            transform.position += velocity.velocity * delta_time;
        }

        Ok(())
    }
}
