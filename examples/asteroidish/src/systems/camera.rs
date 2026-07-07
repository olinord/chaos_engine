use chaos_engine::{
    ChaosReceiver,
    ecs::{system::ChaosSystem, world::ChaosWorld},
    math::Vec2,
};

use crate::{
    components::{camera::CameraComponent, transform::TransformComponent},
    consts::DeviceEvent,
    consts::SpecializedEntities,
};

pub struct CameraSystem {
    message_receiver: Option<ChaosReceiver>,
}

impl CameraSystem {
    pub fn new() -> Self {
        Self {
            message_receiver: None,
        }
    }
}

impl ChaosSystem for CameraSystem {
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        // create a camera component
        world
            .spawn()
            .with(CameraComponent::new(
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 0.0),
                0f32,
            ))
            .specialized(SpecializedEntities::Camera)
            .build();
        self.message_receiver = Some(world.register_for_trigger(DeviceEvent::Resized));
        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        // focus on the ship
        let ship_transform: Option<Vec2> = world
            .get_specialized_entity_component(SpecializedEntities::Ship)
            .map(|transform: &TransformComponent| transform.position);

        let camera_component: Option<&mut CameraComponent> =
            world.get_specialized_entity_component_mut(SpecializedEntities::Camera);

        if let (Some(ship_transform), Some(camera_component)) = (ship_transform, camera_component) {
            camera_component.set_eye(ship_transform);
            camera_component.set_target(ship_transform);

            let message = self.message_receiver.as_mut().unwrap().receive();
            if let Some(message) = message {
                if let Some(width) = message.get::<u32>("width") {
                    if let Some(height) = message.get::<u32>("height") {
                        camera_component.set_aspect_ratio(width as f32 / height as f32);
                    }
                }
            }
        }

        Ok(())
    }
}
