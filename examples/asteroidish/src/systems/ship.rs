use chaos_engine::{
    ChaosReceiver,
    ecs::{EntityID, system::ChaosSystem, world::ChaosWorld},
    math::{Vec2, matrix::Mat3},
    rendering::rendering_system::ChaosRenderableContainer,
};

use crate::components::{
    bounds::BoundingCircle, transform::TransformComponent, velocity::VelocityComponent,
};

use crate::renderables::ship::ShipRenderable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShipEvent {
    Thrust,
    Break,
    Fire,
    RotateLeft,
    RotateRight,
}

pub struct ShipSystem {
    thrust_receiver: Option<ChaosReceiver>,
    break_receiver: Option<ChaosReceiver>,
    fire_receiver: Option<ChaosReceiver>,
    rotate_left_receiver: Option<ChaosReceiver>,
    rotate_right_receiver: Option<ChaosReceiver>,
    ship_entity: Option<EntityID>,
}

impl ShipSystem {
    pub fn new() -> Self {
        Self {
            thrust_receiver: None,
            break_receiver: None,
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

    fn is_rotating_right(&mut self) -> bool {
        match self.rotate_right_receiver.as_mut() {
            Some(receiver) => receiver.receive().is_some(),
            None => false,
        }
    }

    fn is_thrusting(&mut self) -> bool {
        match self.thrust_receiver.as_mut() {
            Some(receiver) => receiver.receive().is_some(),
            None => false,
        }
    }

    fn is_breaking(&mut self) -> bool {
        match self.break_receiver.as_mut() {
            Some(receiver) => receiver.receive().is_some(),
            None => false,
        }
    }
}

impl ChaosSystem for ShipSystem {
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        self.thrust_receiver = Some(world.register_for_trigger(ShipEvent::Thrust));
        self.break_receiver = Some(world.register_for_trigger(ShipEvent::Break));
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
        let query = world.query_for_entity::<(&mut TransformComponent, &mut VelocityComponent)>(
            self.ship_entity.unwrap(),
        );

        if query.is_none() {
            return Err("Failed to query ship components");
        }

        let (transform_component, velocity_component) = query.unwrap();

        if self.is_rotating_left() {
            transform_component.rotation -= 2.0 * delta_time; // Rotate left 
        }
        if self.is_rotating_right() {
            transform_component.rotation += 2.0 * delta_time; // Rotate right
        }

        if self.is_thrusting() {
            let thrust_amount = 1.0;
            let thrust =
                Mat3::rotation(transform_component.rotation) * Vec2::new(0.0, -1.0) * thrust_amount;
            velocity_component.velocity += thrust * delta_time; // Apply thrust
        }
        if self.is_breaking() {
            let break_amount = -0.5;
            let thrust =
                Mat3::rotation(transform_component.rotation) * Vec2::new(0.0, -1.0) * break_amount;
            velocity_component.velocity += thrust * delta_time; // Apply break
        }
        Ok(())
    }
}
