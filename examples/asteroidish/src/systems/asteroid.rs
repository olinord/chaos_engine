use chaos_engine::{
    BufferContents, ChaosReceiver, Vertex,
    ecs::{system::ChaosSystem, world::ChaosWorld},
    log,
    math::{Vec2, Vec3, matrix::Mat4},
    rendering::{
        context::ChaosRenderContext,
        draw_command::{ChaosDrawCommand, ChaosDrawQueue, ChaosRenderPhase},
        effect::ChaosEffect,
        effect_factory::{EffectFactory, EffectUsage},
        multi_draw_indirect_buffer::ChaosMultiDrawIndirectBuffer,
        rendering_system::ChaosRenderSystem,
    },
};
use rand::random_range;
use std::sync::{Arc, Mutex};
use vulkano::pipeline::graphics::input_assembly::PrimitiveTopology;

use crate::{
    components::{
        asteroid::AsteroidComponent, camera::CameraComponent, camera::GpuCameraData,
        health::HealthComponent, physics::PhysicsComponent, shape::ShapeComponent,
        transform::TransformComponent,
    },
    consts::SpecializedEntities,
};

#[derive(BufferContents, Vertex, Copy, Clone)]
#[repr(C)]
struct AsteroidVertex {
    #[format(R32G32_SFLOAT)]
    position: Vec2,
}

pub struct AsteroidSystem {
    location_data: Vec<Mat4>,
    effect: Option<Arc<Mutex<ChaosEffect>>>,
    asteroid_mesh_buffer: Option<Arc<Mutex<ChaosMultiDrawIndirectBuffer<AsteroidVertex>>>>,
    asteroids_removed: Option<ChaosReceiver>,
    render_debug_frame_counter: u64,
}

impl AsteroidSystem {
    pub fn new() -> Self {
        Self {
            location_data: Vec::new(),
            effect: None,
            asteroid_mesh_buffer: None,
            asteroids_removed: None,
            render_debug_frame_counter: 0,
        }
    }

    fn update_storage_buffers(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        self.location_data = world
            .query::<(&TransformComponent, &AsteroidComponent)>()
            .map_err(|_| "Failed to retrieve asteroids")?
            .map(|(_, (transform, _))| transform.as_mat4())
            .collect();
        Ok(())
    }

    fn shared_effect(&self) -> Result<Arc<Mutex<ChaosEffect>>, &'static str> {
        self.effect
            .as_ref()
            .cloned()
            .ok_or("Asteroid effect is not initialized")
    }

    fn shared_mesh_buffer(
        &self,
    ) -> Result<Arc<Mutex<ChaosMultiDrawIndirectBuffer<AsteroidVertex>>>, &'static str> {
        self.asteroid_mesh_buffer
            .as_ref()
            .cloned()
            .ok_or("Asteroid mesh buffer is not initialized")
    }

    fn with_effect_mut<T>(
        &self,
        func: impl FnOnce(&mut ChaosEffect) -> Result<T, &'static str>,
    ) -> Result<T, &'static str> {
        let effect = self.shared_effect()?;
        let mut effect_guard = effect
            .lock()
            .map_err(|_| "Failed to lock asteroid effect")?;
        func(&mut effect_guard)
    }

    fn with_mesh_buffer_mut<T>(
        &self,
        func: impl FnOnce(&mut ChaosMultiDrawIndirectBuffer<AsteroidVertex>) -> Result<T, &'static str>,
    ) -> Result<T, &'static str> {
        let mesh_buffer = self.shared_mesh_buffer()?;
        let mut mesh_guard = mesh_buffer
            .lock()
            .map_err(|_| "Failed to lock asteroid mesh buffer")?;
        func(&mut mesh_guard)
    }

    fn camera_gpu_data(world: &ChaosWorld) -> Result<GpuCameraData, &'static str> {
        let camera_component: &CameraComponent = world
            .get_specialized_entity_component(SpecializedEntities::Camera)
            .ok_or("Failed to retrieve camera component")?;
        Ok(camera_component.get_gpu_data())
    }

    fn reset_asteroid_mesh_buffer(&mut self, world: &mut ChaosWorld) {
        let Ok(asteroid_mesh_buffer) = self.shared_mesh_buffer() else {
            return;
        };

        let mut asteroid_mesh_buffer = asteroid_mesh_buffer
            .lock()
            .expect("Failed to lock asteroid mesh buffer");
        asteroid_mesh_buffer.clear();
        world
            .query::<(&ShapeComponent, &AsteroidComponent)>()
            .map_err(|_| "Failed to retrieve asteroids")
            .unwrap()
            .for_each(|(_, (shape, _))| {
                asteroid_mesh_buffer.add(
                    shape
                        .vertices
                        .iter()
                        .map(|&v| AsteroidVertex { position: v })
                        .collect::<Vec<AsteroidVertex>>(),
                    shape.indices.clone(),
                );
            });
    }
}

impl ChaosSystem for AsteroidSystem {
    fn initialize(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        let mut spheres: Vec<Vec3> = vec![Vec3::new(0.0, 0.0, 2.5)]; // Start with a sphere at the origin with radius 1.0
        let mut count = 0 as usize;
        while count < 10000 {
            let pos = Vec2::new(random_range(-100.0..100.0), random_range(-100.0..100.0));
            let radius = random_range(0.1..1.0);

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
            count += 1;

            spheres.push(Vec3::new(pos.x, pos.y, radius));
            let shape = ShapeComponent::asteroid(
                radius,
                random_range(0.25..0.75),
                random_range(0..1000) as u32,
            );

            world
                .spawn()
                .with(TransformComponent::new().with_position(pos))
                .with(PhysicsComponent::new().with_mass(radius * 1000000f32))
                .with(shape)
                .with(HealthComponent {
                    current: radius,
                    max: radius,
                })
                .with(AsteroidComponent {})
                .build();
            log::info!(
                "Spawned asteroid[{}] at position {:?} with radius {}",
                count,
                pos,
                radius
            );
        }

        self.asteroids_removed = Some(world.subscribe_to_remove::<AsteroidComponent>());

        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        // we won't get an add if there isn't a remove, so we can just reset the mesh buffer if there is a remove
        if self.asteroids_removed.as_mut().unwrap().receive().is_some() {
            self.reset_asteroid_mesh_buffer(world);
        }

        self.update_storage_buffers(world)?;

        Ok(())
    }
}

impl ChaosRenderSystem for AsteroidSystem {
    fn initialize_rendering(
        &mut self,
        world: &mut ChaosWorld,
        ctx: &std::sync::Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let usage = EffectUsage::new("shaders:/asteroids".into())
            .with_primitive_topology(PrimitiveTopology::TriangleList);

        let effect = match EffectFactory::instance().get_effect::<AsteroidVertex>(&usage, ctx) {
            Ok(effect) => effect,
            Err(error) => {
                log::error!("Failed to create asteroid effect: {}", error);
                return Err("Failed to get effect");
            }
        };
        self.effect = Some(Arc::new(Mutex::new(effect)));
        self.asteroid_mesh_buffer = Some(Arc::new(Mutex::new(ChaosMultiDrawIndirectBuffer::new(
            "asteroid-mesh-buffer",
            ctx.clone(),
        ))));

        let asteroid_mesh_buffer = self.shared_mesh_buffer()?;

        world
            .query::<(&TransformComponent, &AsteroidComponent, &ShapeComponent)>()
            .map_err(|_| "Failed to retrieve asteroids")?
            .for_each(|(_, (transform, _, shape))| {
                self.location_data.push(transform.as_mat4());
                asteroid_mesh_buffer
                    .lock()
                    .expect("Failed to lock asteroid mesh buffer")
                    .add(
                        shape
                            .vertices
                            .iter()
                            .map(|&v| AsteroidVertex { position: v })
                            .collect::<Vec<AsteroidVertex>>(),
                        shape.indices.clone(),
                    );
            });

        self.with_effect_mut(|effect| {
            effect
                .set_storage_data_vec(0, 1, vec![Mat4::zero()])
                .map_err(|_| "Failed to set storage data for asteroid effect")
        })?;

        let gpu_data = Self::camera_gpu_data(world)?;
        self.with_effect_mut(|effect| {
            effect
                .set_uniform_data(0, 0, gpu_data)
                .map_err(|_| "Failed to set uniform data for asteroid effect")
        })?;

        Ok(())
    }

    fn prepare_rendering(
        &mut self,
        world: &mut ChaosWorld,
        draw_queue: &mut ChaosDrawQueue,
        _ctx: &std::sync::Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let gpu_data = Self::camera_gpu_data(world)?;
        let effect = self.shared_effect()?;
        let asteroid_mesh_buffer = self.shared_mesh_buffer()?;
        let location_data = self.location_data.clone();
        let model_count = location_data.len();

        self.with_effect_mut(|effect| {
            effect
                .set_uniform_data(0, 0, gpu_data)
                .map_err(|_| "Failed to set uniform data for asteroid effect")?;
            effect
                .set_storage_data_vec(0, 1, location_data)
                .map_err(|_| "Failed to set storage data for asteroid effect")
        })?;

        let (indirect_draw_count, span_count, mesh_is_empty) =
            self.with_mesh_buffer_mut(|mesh_buffer| {
                mesh_buffer
                    .flush()
                    .map_err(|_| "Failed to flush asteroid mesh buffer")?;
                Ok((
                    mesh_buffer.draw_count(),
                    mesh_buffer.spans().len() as u32,
                    mesh_buffer.is_empty(),
                ))
            })?;

        self.render_debug_frame_counter = self.render_debug_frame_counter.wrapping_add(1);
        if self.render_debug_frame_counter % 120 == 0 {
            log::debug!(
                "Asteroid render sanity: models={} indirect_draws={} spans={} mesh_empty={}",
                model_count,
                indirect_draw_count,
                span_count,
                mesh_is_empty
            );
        }

        let effect_hash = self.with_effect_mut(|effect| Ok(effect.hash))?;

        draw_queue.push(ChaosDrawCommand {
            render_phase: ChaosRenderPhase::Opaque,
            effect_hash,
            draw_function: Box::new(move |command_buffer| {
                let effect_guard = effect
                    .lock()
                    .map_err(|_| "Failed to lock asteroid effect")?;
                effect_guard
                    .bind_descriptor_sets(command_buffer)
                    .map_err(|_| "Failed to bind descriptor sets for asteroid effect")?;
                command_buffer
                    .bind_pipeline_graphics(effect_guard.pipeline())
                    .map_err(|_| "Failed to bind asteroid graphics pipeline")?;

                let mesh_guard = asteroid_mesh_buffer
                    .lock()
                    .map_err(|_| "Failed to lock asteroid mesh buffer")?;
                mesh_guard.draw(command_buffer);
                Ok(())
            }),
        });
        Ok(())
    }
}
