use std::time::{Duration, Instant};

use chaos_engine::{
    ecs::{EntityID, system::ChaosSystem, world::ChaosWorld},
    math::Vec2,
    rendering::rendering_system::ChaosRenderableContainer,
};
use rand::random_range;

use crate::{
    components::{
        bounds::BoundingCircle, transform::TransformComponent, velocity::VelocityComponent,
    },
    renderables::asteroid::AsteroidRenderable,
};

pub struct AsteroidSystem {
    last_update: Instant,
    spawned_asteroids: Vec<EntityID>,
}

impl AsteroidSystem {
    pub fn new() -> Self {
        Self {
            last_update: Instant::now(),
            spawned_asteroids: Vec::new(),
        }
    }
}

impl ChaosSystem for AsteroidSystem {
    fn initialize(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        if self.last_update.elapsed() < Duration::from_millis(1000) {
            return Ok(());
        }
        self.last_update = Instant::now();

        if self.spawned_asteroids.len() >= 10 {
            for entity_id in &self.spawned_asteroids {
                world.despawn(*entity_id);
            }
            self.spawned_asteroids.clear();
        }

        self.spawned_asteroids.push(
            world
                .spawn()
                .with(
                    TransformComponent::new()
                        .with_position(Vec2::new(random_range(-5.0..5.0), random_range(-5.0..5.0))),
                )
                .with(VelocityComponent::new())
                .with(BoundingCircle::new())
                .with(ChaosRenderableContainer::new(AsteroidRenderable::new(
                    Vec2::new(random_range(0.2..1.5), random_range(0.2..1.5)),
                    random_range(0.1..1.0),
                )))
                .build(),
        );
        Ok(())
    }
}
