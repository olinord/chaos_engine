use chaos_engine::{
    ChaosReceiver,
    ecs::{system::ChaosSystem, world::ChaosWorld},
    math::Vec2,
};

use crate::{
    components::{
        camera::CameraComponent, transform::TransformComponent, velocity::VelocityComponent,
    },
    consts::{DeviceEvent, SpecializedEntities},
};

pub struct CameraSystem {
    message_receiver: Option<ChaosReceiver>,
    width: u32,
    height: u32,
}

impl CameraSystem {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            message_receiver: None,
            width,
            height,
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
                self.width as f32 / self.height as f32,
            ))
            .specialized(SpecializedEntities::Camera)
            .build();
        self.message_receiver = Some(world.register_for_trigger(DeviceEvent::Resized));
        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        let delta_time = world.get_time().delta_time();

        // focus on the ship
        let ship_transform: Option<Vec2> = world
            .get_specialized_entity_component(SpecializedEntities::Ship)
            .map(|transform: &TransformComponent| transform.position);

        let ship_velocity: Option<Vec2> = world
            .get_specialized_entity_component(SpecializedEntities::Ship)
            .map(|velocity: &VelocityComponent| velocity.velocity);

        let camera_component: Option<&mut CameraComponent> =
            world.get_specialized_entity_component_mut(SpecializedEntities::Camera);

        if let (Some(ship_transform), Some(camera_component), Some(ship_velocity)) =
            (ship_transform, camera_component, ship_velocity)
        {
            let target = ship_transform + ship_velocity;
            let current_camera_position = Vec2::lerp(
                &camera_component.eye.xy(),
                &target.xy(),
                delta_time * ship_velocity.length(),
            );

            camera_component.update(current_camera_position, current_camera_position, 0f32);

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
