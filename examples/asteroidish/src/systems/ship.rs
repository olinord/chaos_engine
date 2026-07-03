use chaos_engine::{
    ChaosReceiver,
    ecs::{EntityID, system::ChaosSystem, world::ChaosWorld},
    math::Vec2,
    rendering::rendering_system::ChaosRenderableContainer,
};

use crate::components::{
    bounds::BoundingCircle, transform::TransformComponent, velocity::VelocityComponent,
};

use crate::renderables::ship::ShipRenderable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShipEvent {
    Thrust,
    Fire,
    RotateLeft,
    RotateRight,
}

pub struct ShipSystem {
    thrust_receiver: Option<ChaosReceiver>,
    fire_receiver: Option<ChaosReceiver>,
    rotate_left_receiver: Option<ChaosReceiver>,
    rotate_right_receiver: Option<ChaosReceiver>,
    ship_entity: Option<EntityID>,
}

impl ShipSystem {
    pub fn new() -> Self {
        Self {
            thrust_receiver: None,
            fire_receiver: None,
            rotate_left_receiver: None,
            rotate_right_receiver: None,
            ship_entity: None,
        }
    }

    fn is_rotating_left(&mut self) -> bool {
        match self.rotate_left_receiver.as_mut() {
            Some(receiver) => receiver.receive().is_some(),
            None => false,
        }
    }
}

impl ChaosSystem for ShipSystem {
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        self.thrust_receiver = Some(world.register_for_trigger(ShipEvent::Thrust));
        self.fire_receiver = Some(world.register_for_trigger(ShipEvent::Fire));
        self.rotate_left_receiver = Some(world.register_for_trigger(ShipEvent::RotateLeft));
        self.rotate_right_receiver = Some(world.register_for_trigger(ShipEvent::RotateRight));

        // create the ship
        self.ship_entity = Some(
            world
                .spawn()
                .with(TransformComponent::new())
                .with(VelocityComponent::new())
                .with(BoundingCircle::new())
                .with(ChaosRenderableContainer::new(ShipRenderable::new()))
                .build(),
        );

        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        let delta_time = world.get_time().delta_time();
        let transform_component =
            match world.get_component_mut::<TransformComponent>(self.ship_entity.unwrap()) {
                Some(component) => component,
                None => return Err("Failed to get transform component for ship entity")?,
            };

        if self.is_rotating_left() {
            transform_component.rotation -= 1.0 * delta_time; // Rotate left 
        }
        Ok(())
    }
}
