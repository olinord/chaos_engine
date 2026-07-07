use std::sync::Arc;

use chaos_engine::ecs::EntityID;
use chaos_engine::ecs::world::ChaosWorld;
use chaos_engine::math::Vec2;
use chaos_engine::math::matrix::Mat4;
use chaos_engine::rendering::buffer::{ChaosBufferMemoryType, ChaosBufferUsage};
use chaos_engine::rendering::effect_factory::{EffectFactory, EffectUsage};
use chaos_engine::rendering::rendering_system::{ChaosRenderContext, ChaosRenderableTrait};
use chaos_engine::rendering::{buffer::ChaosBuffer, effect::ChaosEffect};
use chaos_engine::{BufferContents, Vertex};
use rand::random_range;
use vulkano::ValidationError;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::pipeline::graphics::input_assembly::PrimitiveTopology;

use crate::components::camera::CameraComponent;
use crate::components::transform::TransformComponent;
use crate::consts::SpecializedEntities;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct ShipVertex {
    #[format(R32G32_SFLOAT)]
    position: Vec2,
}

pub struct AsteroidRenderable {
    effect: Option<ChaosEffect>,
    buffer: Option<ChaosBuffer>,
    points: Vec<Vec2>,
    push_constants: PushConstants,
}

#[derive(BufferContents)]
#[repr(C)]
struct PushConstants {
    model: Mat4,
}

impl PushConstants {
    fn from_model(model: Mat4) -> Self {
        Self { model }
    }
}

#[derive(BufferContents)]
#[repr(C)]
struct PerFrameUniforms {
    projection: Mat4,
    view: Mat4,
}

impl AsteroidRenderable {
    pub fn new(radius: Vec2, roughness: f32) -> Self {
        // generate the points for the asteroid shape
        let segments = random_range(12..128); // random number of segments between 128 and 256

        let noise_function = |x: f32| -> f32 {
            let mut noise = 0.0;
            let mut frequency = roughness;
            let mut amplitude = 1.0;
            for _ in 0..4 {
                noise += amplitude * (x * frequency).sin();
                frequency *= 2.0;
                amplitude *= 0.5;
            }
            noise
        };

        let start_point = random_range(-1000.0f32..1000.0f32);

        let radii = (0..segments)
            .map(|i| radius + radius * noise_function(start_point + i as f32 / segments as f32))
            .collect::<Vec<Vec2>>();

        // smooth the radii to create a more natural shape
        let smooth_radii: Vec<Vec2> = radii
            .iter()
            .enumerate()
            .map(|(i, &r)| {
                let prev = radii[(i + segments - 1) % segments];
                let next = radii[(i + 1) % segments];
                (prev + r * 2.0 + next) / 4.0
            })
            .collect();

        let points: Vec<Vec2> = smooth_radii
            .iter()
            .enumerate()
            .map(|(i, &r)| {
                let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                Vec2::new(r.x * angle.cos(), r.y * angle.sin())
            })
            .collect();

        Self {
            effect: None,
            buffer: None,
            points,
            push_constants: PushConstants::from_model(Mat4::identity()),
        }
    }
}

impl ChaosRenderableTrait for AsteroidRenderable {
    fn initialize(
        &mut self,
        world: &ChaosWorld,
        entity_id: EntityID,
        rendering_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let usage = EffectUsage::new("shaders:/line".into())
            .with_primitive_topology(PrimitiveTopology::TriangleFan);

        let effect = EffectFactory::instance().get_effect::<ShipVertex>(&usage, rendering_context);
        if let Err(_error) = effect {
            let error_string = format!("Failed to create triangle pipeline: {}", _error);
            println!("{}", error_string);
            return Err("boohoo");
        }

        let mut buffer = ChaosBuffer::new(
            "asteroid-vertex-buffer".into(),
            ChaosBufferUsage::VertexBuffer,
            ChaosBufferMemoryType::PreferDevice,
            rendering_context.clone(),
        );

        buffer
            .set_data(self.points.clone())
            .map_err(|_| "Failed to set asteroid vertex buffer data")?;

        self.push_constants.model = world
            .get_component::<TransformComponent>(entity_id)
            .map(|tc| tc.as_matrix())
            .unwrap_or(Mat4::identity());

        self.effect = Some(effect.unwrap());
        self.buffer = Some(buffer);

        Ok(())
    }

    fn update(
        &mut self,
        world: &ChaosWorld,
        _entity_id: EntityID,
        _render_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let camera_component = world
            .get_specialized_entity_component(SpecializedEntities::Camera)
            .map(|component: &CameraComponent| {
                (component.projection_matrix, component.view_matrix)
            });

        if let None = camera_component {
            return Err("Camera entity not found");
        }

        let camera_component = camera_component.unwrap();

        if let Some(effect) = &mut self.effect {
            effect
                .set_uniform_data(
                    0,
                    0,
                    vec![PerFrameUniforms {
                        projection: camera_component.0,
                        view: camera_component.1,
                    }],
                )
                .map_err(|_| "Failed to set asteroid MVP uniform data")?;
        }

        Ok(())
    }

    fn add_to_command_buffer(
        &self,
        world: &ChaosWorld,
        entity_id: EntityID,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), Box<ValidationError>> {
        if self.effect.is_none() || self.buffer.is_none() {
            return Ok(());
        }
        let buffer = self
            .buffer
            .as_ref()
            .unwrap()
            .buffer()
            .unwrap()
            .as_ref()
            .clone();

        let effect = self.effect.as_ref().unwrap();

        let transform_component = world.get_component::<TransformComponent>(entity_id);

        effect.bind_descriptor_sets(command_buffer)?;
        effect.bind_push_constants(
            command_buffer,
            0,
            PushConstants::from_model(
                transform_component
                    .map(|tc| tc.as_matrix())
                    .unwrap_or(Mat4::identity()),
            ),
        )?;
        command_buffer
            .bind_pipeline_graphics(effect.pipeline())?
            .bind_vertex_buffers(0, buffer)?;
        unsafe {
            command_buffer.draw(self.points.len() as u32, 1, 0, 0)?;
        }
        Ok(())
    }
}
