use chaos_engine::ecs::{system::ChaosSystem, world::ChaosWorld};

pub struct ImpactSystem {}

impl ChaosSystem for ImpactSystem {
    fn initialize(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        Ok(())
    }

    fn update(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        // let mut query = world.query::<(&BoundingCircle, &Position, &mut Velocity)>();

        // for (entity, (bounding_circle, position, velocity)) in query.iter_mut() {
        //     // Check for impact with boundaries or other objects
        //     // If impact occurs, handle it (e.g., change velocity, position, etc.)

        //     for

        // }

        Ok(())
    }
}
