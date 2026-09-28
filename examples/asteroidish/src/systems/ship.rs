use std::sync::{Arc, Mutex};

use chaos_engine::{
    BufferContents, ChaosReceiver, Vertex,
    ecs::{EntityID, system::ChaosSystem, world::ChaosWorld},
    log,
    math::{
        Vec2,
        matrix::{Mat3, Mat4},
    },
    rendering::{
        buffer::ChaosBuffer,
        context::ChaosRenderContext,
        draw_command::{ChaosDrawCommand, ChaosDrawQueue, ChaosRenderPhase},
        effect::ChaosEffect,
        effect_factory::{EffectFactory, EffectUsage},
        rendering_system::ChaosRenderSystem,
    },
};
use vulkano::pipeline::graphics::input_assembly::PrimitiveTopology;

use crate::{
    components::{
        bullet::BulletComponent,
        camera::{CameraComponent, GpuCameraData},
        health::HealthComponent,
        physics::PhysicsComponent,
        shape::ShapeComponent,
        ship::ShipComponent,
        transform::TransformComponent,
    },
    consts::SpecializedEntities,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShipEvent {
    Thrust,
    Break,
    Fire,
    RotateLeft,
    RotateRight,
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct ShipVertex {
    #[format(R32G32_SFLOAT)]
    position: Vec2,
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct BulletVertex {
    #[format(R32G32_SFLOAT)]
    position: Vec2,
}

pub struct ShipSystem {
    thrust_receiver: Option<ChaosReceiver>,
    break_receiver: Option<ChaosReceiver>,
    fire_receiver: Option<ChaosReceiver>,
    rotate_left_receiver: Option<ChaosReceiver>,
    rotate_right_receiver: Option<ChaosReceiver>,
    ship_effect: Option<Arc<Mutex<ChaosEffect>>>,
    bullet_effect: Option<Arc<Mutex<ChaosEffect>>>,
    ship_buffer: Option<Arc<Mutex<ChaosBuffer>>>,
    bullet_buffer: Option<Arc<Mutex<ChaosBuffer>>>,
    bullet_locations: Vec<Mat4>,
    render_debug_frame_counter: u64,
}

impl ShipSystem {
    pub fn new() -> Self {
        Self {
            thrust_receiver: None,
            break_receiver: None,
            fire_receiver: None,
            rotate_left_receiver: None,
            rotate_right_receiver: None,
            ship_effect: None,
            bullet_effect: None,
            ship_buffer: None,
            bullet_buffer: None,
            bullet_locations: Vec::new(),
            render_debug_frame_counter: 0,
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

    fn decrease_bullet_health(&mut self, world: &mut ChaosWorld) {
        let delta_time = world.get_time().delta_time();

        let mut despawned_entities: Vec<EntityID> = Vec::new();

        match world.query::<(&mut HealthComponent, &BulletComponent)>() {
            Err(_) => {
                println!("Failed to query bullet health components");
                return;
            }
            Ok(bullet_entities) => {
                for (entity_id, (health, _)) in bullet_entities {
                    health.current -= 1.0 * delta_time; // Decrease health over time
                    if health.current <= 0.0 {
                        despawned_entities.push(entity_id);
                    }
                }
            }
        }

        for entity_id in despawned_entities {
            world.despawn(entity_id);
        }
    }

    fn camera_gpu_data(world: &ChaosWorld) -> Result<GpuCameraData, &'static str> {
        let camera_component: &CameraComponent = world
            .get_specialized_entity_component(SpecializedEntities::Camera)
            .ok_or("Failed to retrieve camera component")?;
        Ok(camera_component.get_gpu_data())
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
            .with(PhysicsComponent::new().with_mass(1000f32))
            .with(ShipComponent {})
            .with(ShapeComponent::ship())
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

        self.decrease_bullet_health(world);

        let (transform_component, physics_component) = {
            let query = world.query_for_entity::<(&mut TransformComponent, &mut PhysicsComponent)>(
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
            physics_component.velocity += thrust * delta_time; // Apply thrust
        }
        if self.is_breaking() {
            let break_amount = -0.5;
            let thrust = (Mat3::rotation(transform_component.rotation) * Vec2::new(0.0, -1.0))
                * break_amount;
            if Vec2::distance_squared(&physics_component.velocity, &(thrust * delta_time)) < 0.1f32
            {
                physics_component.velocity = Vec2::new(0.0, 0.0);
            } else {
                physics_component.velocity += thrust * delta_time; // Apply break
            }
        }

        let ship_position = transform_component.position;
        let ship_rotation = transform_component.rotation;
        let ship_velocity = physics_component.velocity;

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
                .with(BulletComponent {})
                .with(
                    PhysicsComponent::new()
                        .with_mass(1.0)
                        .with_velocity(initial_velocity),
                )
                .with(HealthComponent {
                    current: 1.0,
                    max: 1.0,
                })
                .build();
        }

        let bullet_transforms = world.query::<(&TransformComponent, &BulletComponent)>();

        self.bullet_locations.clear();
        if let Ok(bullet_entities) = bullet_transforms {
            self.bullet_locations =
                Vec::from_iter(bullet_entities.map(|(_, (transform, _))| transform.as_mat4()));
        }
        Ok(())
    }
}

impl ChaosRenderSystem for ShipSystem {
    fn initialize_rendering(
        &mut self,
        _world: &mut ChaosWorld,
        ctx: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let usage = EffectUsage::new("shaders:/ship".into())
            .with_primitive_topology(PrimitiveTopology::TriangleList);

        match EffectFactory::instance().get_effect::<ShipVertex>(&usage, ctx) {
            Ok(effect) => self.ship_effect = Some(Arc::new(Mutex::new(effect))),
            Err(_error) => {
                let error_string = format!("Failed to create ship pipeline: {}", _error);
                log::error!("{}", error_string);
                return Err("Failed to create ship pipeline");
            }
        }

        match EffectFactory::instance()
            .get_effect::<BulletVertex>(&EffectUsage::new("shaders:/bullet".into()), ctx)
        {
            Ok(effect) => self.bullet_effect = Some(Arc::new(Mutex::new(effect))),
            Err(_error) => {
                let error_string = format!("Failed to create bullet pipeline: {}", _error);
                log::error!("{}", error_string);
                return Err("Failed to create bullet pipeline");
            }
        }

        self.ship_buffer = {
            let mut buffer = ChaosBuffer::new(
                "ship-vertex-buffer".into(),
                chaos_engine::rendering::buffer::ChaosBufferUsage::VertexBuffer,
                chaos_engine::rendering::buffer::ChaosBufferMemoryType::PreferDevice,
                ctx.clone(),
            );

            buffer
                .set_data_from_vec(
                    ShapeComponent::ship()
                        .vertices
                        .iter()
                        .map(|pos| ShipVertex { position: *pos })
                        .collect::<Vec<ShipVertex>>(),
                )
                .map_err(|_| "Failed to set ship vertex buffer data")?;
            Some(Arc::new(Mutex::new(buffer)))
        };

        self.bullet_buffer = {
            let mut buffer = ChaosBuffer::new(
                "bullet-vertex-buffer".into(),
                chaos_engine::rendering::buffer::ChaosBufferUsage::VertexBuffer,
                chaos_engine::rendering::buffer::ChaosBufferMemoryType::PreferDevice,
                ctx.clone(),
            );
            buffer
                .set_data_from_vec(
                    ShapeComponent::bullet()
                        .vertices
                        .iter()
                        .map(|pos| BulletVertex { position: *pos })
                        .collect::<Vec<BulletVertex>>(),
                )
                .map_err(|_| "Failed to set bullet vertex buffer data")?;
            Some(Arc::new(Mutex::new(buffer)))
        };

        Ok(())
    }

    fn prepare_rendering(
        &mut self,
        world: &mut ChaosWorld,
        draw_queue: &mut ChaosDrawQueue,
        _ctx: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let ship_effect = self
            .ship_effect
            .as_ref()
            .cloned()
            .ok_or("Ship effect is not initialized")?;
        let camera_data = Self::camera_gpu_data(world)?;
        let ship_buffer = self
            .ship_buffer
            .as_ref()
            .cloned()
            .ok_or("Ship buffer is not initialized")?;
        let ship_model = world
            .get_specialized_entity_component(SpecializedEntities::Ship)
            .map(|transform: &TransformComponent| transform.as_mat4())
            .ok_or("Failed to retrieve ship transform")?;
        let bullet_effect = self
            .bullet_effect
            .as_ref()
            .cloned()
            .ok_or("Bullet effect is not initialized")?;
        let bullet_buffer = self
            .bullet_buffer
            .as_ref()
            .cloned()
            .ok_or("Bullet buffer is not initialized")?;
        let bullet_locations = self.bullet_locations.clone();
        let ship_effect_hash = ship_effect
            .lock()
            .map_err(|_| "Failed to lock ship effect")?
            .hash;
        let bullet_effect_hash = bullet_effect
            .lock()
            .map_err(|_| "Failed to lock bullet effect")?
            .hash;
        let ship_vertex_count = ship_buffer
            .lock()
            .map_err(|_| "Failed to lock ship buffer")?
            .length();
        let bullet_vertex_count = bullet_buffer
            .lock()
            .map_err(|_| "Failed to lock bullet buffer")?
            .length();
        let bullet_count = world.count::<(&BulletComponent,)>() as u32;

        self.render_debug_frame_counter = self.render_debug_frame_counter.wrapping_add(1);
        if self.render_debug_frame_counter % 120 == 0 {
            log::debug!(
                "Ship render sanity: ship_vertices={} bullets={} bullet_instances={}",
                ship_vertex_count,
                bullet_vertex_count,
                bullet_count
            );
        }

        let ship_draw_command = ChaosDrawCommand {
            render_phase: ChaosRenderPhase::Opaque,
            effect_hash: ship_effect_hash,
            draw_function: Box::new(move |command_buffer| {
                let mut ship_effect = ship_effect
                    .lock()
                    .map_err(|_| "Failed to lock ship effect")?;
                let ship_buffer = ship_buffer
                    .lock()
                    .map_err(|_| "Failed to lock ship buffer")?;
                ship_effect
                    .set_uniform_data(0, 0, camera_data.clone())
                    .map_err(|_| "Failed to set uniform data for ship effect")?;
                ship_effect
                    .set_uniform_data(0, 1, ship_model)
                    .map_err(|_| "Failed to set uniform data for ship model")?;
                ship_effect
                    .bind_descriptor_sets(command_buffer)
                    .map_err(|_| "Failed to bind descriptor sets for ship effect")?;
                command_buffer
                    .bind_pipeline_graphics(ship_effect.pipeline())
                    .map_err(|_| "Failed to bind ship graphics pipeline")?
                    .bind_vertex_buffers(
                        0,
                        ship_buffer
                            .buffer()
                            .ok_or("Ship buffer not bound")?
                            .as_ref()
                            .clone(),
                    )
                    .map_err(|_| "Failed to bind ship vertex buffer")?;
                unsafe {
                    let result = command_buffer.draw(ship_buffer.length(), 1, 0, 0);
                    if let Err(e) = result {
                        println!("Failed to draw ship: {:?}", e);
                        return Err("Failed to draw ship");
                    }
                }
                Ok(())
            }),
        };
        if bullet_count > 0 {
            let bullet_draw_command = ChaosDrawCommand {
                render_phase: ChaosRenderPhase::Opaque,
                effect_hash: bullet_effect_hash,
                draw_function: Box::new(move |command_buffer| {
                    let mut bullet_effect = bullet_effect
                        .lock()
                        .map_err(|_| "Failed to lock bullet effect")?;
                    let bullet_buffer = bullet_buffer
                        .lock()
                        .map_err(|_| "Failed to lock bullet buffer")?;
                    bullet_effect
                        .set_uniform_data(0, 0, camera_data.clone())
                        .map_err(|_| "Failed to set uniform data for bullet effect")?;
                    bullet_effect
                        .set_storage_data_vec(0, 1, bullet_locations)
                        .map_err(|_| "Failed to set storage data for bullet effect")?;
                    bullet_effect
                        .bind_descriptor_sets(command_buffer)
                        .map_err(|_| "Failed to bind descriptor sets for bullet effect")?;

                    command_buffer
                        .bind_pipeline_graphics(bullet_effect.pipeline())
                        .map_err(|_| "Failed to bind bullet graphics pipeline")?
                        .bind_vertex_buffers(
                            0,
                            bullet_buffer
                                .buffer()
                                .ok_or("Bullet buffer not bound")?
                                .as_ref()
                                .clone(),
                        )
                        .map_err(|_| "Failed to bind bullet vertex buffer")?;

                    unsafe {
                        command_buffer
                            .draw(bullet_buffer.length(), bullet_count, 0, 0)
                            .map_err(|_| "Failed to draw bullets")?;
                    }
                    Ok(())
                }),
            };
            draw_queue.push(bullet_draw_command);
        }

        draw_queue.push(ship_draw_command);
        Ok(())
    }
}
