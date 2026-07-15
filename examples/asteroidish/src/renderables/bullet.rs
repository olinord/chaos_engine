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
use vulkano::ValidationError;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

use crate::components::camera::CameraComponent;
use crate::components::shape::ShapeComponent;
use crate::components::transform::TransformComponent;
use crate::consts::SpecializedEntities;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct BulletVertex {
    #[format(R32G32_SFLOAT)]
    position: Vec2,
}

pub struct BulletRenderable {
    effect: Option<ChaosEffect>,
    buffer: Option<ChaosBuffer>,
    vertex_count: u32,
    push_constants: BulletPushConstants,
}

#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct BulletPushConstants {
    model: Mat4,
}

impl BulletPushConstants {
    fn from_model(model: Mat4) -> Self {
        Self { model }
    }
}

#[derive(BufferContents)]
#[repr(C)]
struct TriangleMvp {
    projection: Mat4,
    view: Mat4,
}

impl BulletRenderable {
    pub fn new() -> Self {
        Self {
            effect: None,
            buffer: None,
            vertex_count: 0,
            push_constants: BulletPushConstants::from_model(Mat4::identity()),
        }
    }
}

impl ChaosRenderableTrait for BulletRenderable {
    fn initialize(
        &mut self,
        world: &ChaosWorld,
        entity_id: EntityID,
        rendering_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let usage = EffectUsage::new("shaders:/triangle".into());

        let effect =
            EffectFactory::instance().get_effect::<BulletVertex>(&usage, rendering_context);
        if let Err(_error) = effect {
            let error_string = format!("Failed to create triangle pipeline: {}", _error);
            println!("{}", error_string);
            return Err("boohoo");
        }

        let points = world
            .get_component::<ShapeComponent>(entity_id)
            .map(|shape| {
                shape
                    .shape
                    .iter()
                    .flat_map(|tri| [tri.a, tri.b, tri.c])
                    .collect::<Vec<Vec2>>()
            })
            .unwrap_or(Vec::new());
        self.vertex_count = points.len() as u32;

        let mut buffer = ChaosBuffer::new(
            "triangle-vertex-buffer".into(),
            ChaosBufferUsage::VertexBuffer,
            ChaosBufferMemoryType::PreferDevice,
            rendering_context.clone(),
        );
        buffer
            .set_data(points)
            .map_err(|_| "Failed to set triangle vertex buffer data")?;

        let transform_component = world
            .get_component::<TransformComponent>(entity_id)
            .unwrap();

        self.effect = Some(effect.unwrap());
        self.buffer = Some(buffer);
        self.push_constants = BulletPushConstants::from_model(transform_component.as_mat4());
        Ok(())
    }

    fn update(
        &mut self,
        world: &ChaosWorld,
        entity_id: EntityID,
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

        self.effect
            .as_mut()
            .unwrap()
            .set_uniform_data(
                0,
                0,
                vec![TriangleMvp {
                    projection: camera_component.0,
                    view: camera_component.1,
                }],
            )
            .map_err(|_| "Failed to set uniform data")?;

        let transform_component = world
            .get_component::<TransformComponent>(entity_id)
            .unwrap();
        self.push_constants.model = transform_component.as_mat4();

        Ok(())
    }

    fn add_to_command_buffer(
        &self,
        _world: &ChaosWorld,
        _entity_id: EntityID,
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

        effect.bind_descriptor_sets(command_buffer)?;
        effect.bind_push_constants(command_buffer, 0, self.push_constants)?;

        command_buffer
            .bind_pipeline_graphics(effect.pipeline())?
            .bind_vertex_buffers(0, buffer)?;
        unsafe {
            command_buffer.draw(self.vertex_count, 1, 0, 0)?;
        }

        Ok(())
    }
}
