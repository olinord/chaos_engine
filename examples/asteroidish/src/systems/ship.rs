use chaos_engine::{
    ChaosReceiver,
    ecs::{system::ChaosSystem, world::ChaosWorld},
    math::{Vec2, matrix::Mat3},
    rendering::rendering_system::ChaosRenderableContainer,
};

use crate::{
    components::{
        shape::ShapeComponent, transform::TransformComponent, velocity::VelocityComponent,
    },
    consts::SpecializedEntities,
    renderables::bullet::BulletRenderable,
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
}

impl ShipSystem {
    pub fn new() -> Self {
        Self {
            thrust_receiver: None,
            break_receiver: None,
            fire_receiver: None,
            rotate_left_receiver: None,
            rotate_right_receiver: None,
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

    fn is_firing(&mut self) -> bool {
        match self.fire_receiver.as_mut() {
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
        world
            .spawn()
            .with(TransformComponent::new())
            .with(VelocityComponent::new())
            .with(ShapeComponent::ship())
            .with(ChaosRenderableContainer::new(ShipRenderable::new()))
            .specialized(SpecializedEntities::Ship)
            .build();

        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        let delta_time = world.get_time().delta_time();
        let ship_entity = world.get_specialized_entity(SpecializedEntities::Ship);

        if let None = ship_entity {
            return Err("Ship entity not found");
        }

        let (transform_component, velocity_component) = {
            let query = world
                .query_for_entity::<(&mut TransformComponent, &mut VelocityComponent)>(
                    ship_entity.unwrap(),
                );

            if query.is_none() {
                return Err("Failed to query ship components");
            }
            query.unwrap()
        };

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
            let thrust = (Mat3::rotation(transform_component.rotation) * Vec2::new(0.0, -1.0))
                * break_amount;
            if Vec2::distance_squared(&velocity_component.velocity, &(thrust * delta_time)) < 0.1f32
            {
                velocity_component.velocity = Vec2::new(0.0, 0.0);
            } else {
                velocity_component.velocity += thrust * delta_time; // Apply break
            }
        }

        let ship_position = transform_component.position;
        let ship_rotation = transform_component.rotation;
        let ship_velocity = velocity_component.velocity;

        if self.is_firing() {
            let firing_speed = 5.0;
            let firing_direction = Mat3::rotation(ship_rotation) * Vec2::new(0.0, -1.0);
            let initial_position = ship_position + firing_direction * 0.5; // Offset the bullet's initial position
            let initial_velocity = ship_velocity + firing_direction * firing_speed;
            world
                .spawn()
                .with(TransformComponent {
                    position: initial_position,
                    rotation: ship_rotation,
                    scale: Vec2::one(),
                })
                .with(VelocityComponent {
                    velocity: initial_velocity,
                })
                .with(ShapeComponent::bullet())
                .with(ChaosRenderableContainer::new(BulletRenderable::new()))
                .build();
        }
        Ok(())
    }
}
