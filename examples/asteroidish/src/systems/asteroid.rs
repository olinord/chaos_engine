use chaos_engine::{
    ecs::{EntityID, system::ChaosSystem, world::ChaosWorld},
    math::{Vec2, Vec3},
    rendering::rendering_system::ChaosRenderableContainer,
};
use rand::random_range;

use crate::{
    components::{
        shape::ShapeComponent, transform::TransformComponent, velocity::VelocityComponent,
    },
    renderables::asteroid::AsteroidRenderable,
};

pub struct AsteroidSystem {
    spawned_asteroids: Vec<EntityID>,
}

impl AsteroidSystem {
    pub fn new() -> Self {
        Self {
            spawned_asteroids: Vec::new(),
        }
    }
}

impl ChaosSystem for AsteroidSystem {
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        let mut spheres: Vec<Vec3> = vec![Vec3::new(0.0, 0.0, 2.5)]; // Start with a sphere at the origin with radius 1.0

        while self.spawned_asteroids.len() < 10 {
            let pos = Vec2::new(random_range(-100.0..100.0), random_range(-100.0..100.0));
            let radius = random_range(1.0..15.0);

            let mut collision = false;
            for sphere in &spheres {
                let distance = (pos - Vec2::new(sphere.x, sphere.y)).length_squared();
                if distance < (radius + sphere.z) * (radius + sphere.z) {
                    collision = true;
                    break;
                }
            }
            if collision {
                continue;
            }

            spheres.push(Vec3::new(pos.x, pos.y, radius));
            self.spawned_asteroids.push(
                world
                    .spawn()
                    .with(TransformComponent::new().with_position(pos))
                    .with(VelocityComponent::new())
                    .with(ShapeComponent::asteroid(
                        radius,
                        random_range(0.25..0.75),
                        random_range(0..1000) as u32,
                    ))
                    .with(ChaosRenderableContainer::new(AsteroidRenderable::new()))
                    .build(),
            );
        }
        Ok(())
    }

    fn update(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        Ok(())
    }
}
