use chaos_engine::{
    ecs::{system::ChaosSystem, world::ChaosWorld},
    math::Vec2,
};

use crate::components::camera::CameraComponent;

pub struct CameraSystem {
    camera_entity: Option<chaos_engine::ecs::EntityID>,
}

impl CameraSystem {
    pub fn new() -> Self {
        Self {
            camera_entity: None,
        }
    }
}

impl ChaosSystem for CameraSystem {
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        // create a camera component
        self.camera_entity = Some(
            world
                .spawn()
                .with(CameraComponent::new(
                    Vec2::new(0.0, 0.0),
                    Vec2::new(0.0, 0.0),
                    0f32,
                ))
                .build(),
        );
        Ok(())
    }

    fn update(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        Ok(())
    }
}
