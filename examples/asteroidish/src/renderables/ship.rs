use std::sync::Arc;

use chaos_engine::ecs::EntityID;
use chaos_engine::ecs::world::ChaosWorld;
use chaos_engine::math::matrix::Mat4;
use chaos_engine::math::{Vec2, Vec4};
use chaos_engine::rendering::buffer::{ChaosBufferMemoryType, ChaosBufferUsage};
use chaos_engine::rendering::effect_factory::{EffectFactory, EffectUsage};
use chaos_engine::rendering::rendering_system::{ChaosRenderContext, ChaosRenderableTrait};
use chaos_engine::rendering::{buffer::ChaosBuffer, effect::ChaosEffect};
use chaos_engine::{BufferContents, Vertex};
use vulkano::ValidationError;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

use crate::components::transform::TransformComponent;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct ShipVertex {
    #[format(R32G32_SFLOAT)]
    position: Vec2,
    #[format(R32G32B32A32_SFLOAT)]
    color: Vec4,
}

pub struct ShipRenderable {
    effect: Option<ChaosEffect>,
    buffer: Option<ChaosBuffer>,
}

#[derive(BufferContents)]
#[repr(C)]
struct TrianglePushConstants {
    model: Mat4,
}

impl TrianglePushConstants {
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

impl ShipRenderable {
    pub fn new() -> Self {
        Self {
            effect: None,
            buffer: None,
        }
    }
}

impl ChaosRenderableTrait for ShipRenderable {
    fn initialize(
        &mut self,
        rendering_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let vertices = vec![
            ShipVertex {
                position: [0.0, -0.5].into(),
                color: [1.0, 0.2, 0.1, 1.0].into(),
            },
            ShipVertex {
                position: [0.5, 0.5].into(),
                color: [0.1, 0.8, 0.25, 1.0].into(),
            },
            ShipVertex {
                position: [-0.5, 0.5].into(),
                color: [0.1, 0.35, 1.0, 1.0].into(),
            },
        ];

        let usage = EffectUsage::new("shaders:/triangle".into());

        let effect = EffectFactory::instance().get_effect::<ShipVertex>(&usage, rendering_context);
        if let Err(_error) = effect {
            let error_string = format!("Failed to create triangle pipeline: {}", _error);
            println!("{}", error_string);
            return Err("boohoo");
        }
        let mut effect = effect.unwrap();
        effect
            .set_uniform_data(
                0,
                0,
                vec![TriangleMvp {
                    projection: Mat4::perspective(std::f32::consts::PI / 2.0, 1.0, 0.1, 20.0),
                    view: Mat4::look_at(
                        &[0.0, 0.0, -5.0].into(),
                        &[0.0, 0.0, 0.0].into(),
                        &[0.0, 1.0, 0.0].into(),
                    ),
                }],
            )
            .map_err(|_| "Failed to set triangle MVP uniform data")?;

        let mut buffer = ChaosBuffer::new(
            "triangle-vertex-buffer".into(),
            ChaosBufferUsage::VertexBuffer,
            ChaosBufferMemoryType::PreferDevice,
            rendering_context.clone(),
        );
        buffer
            .set_data(vertices)
            .map_err(|_| "Failed to set triangle vertex buffer data")?;

        self.effect = Some(effect);
        self.buffer = Some(buffer);
        println!("Initialized ship renderable");
        Ok(())
    }

    fn add_to_command_buffer(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        world: &ChaosWorld,
        entity: &EntityID,
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

        let transform_component = world.get_component::<TransformComponent>(*entity);

        effect.bind_descriptor_sets(command_buffer)?;
        effect.bind_push_constants(
            command_buffer,
            0,
            TrianglePushConstants::from_model(
                transform_component
                    .map(|tc| tc.as_matrix())
                    .unwrap_or(Mat4::identity()),
            ),
        )?;
        command_buffer
            .bind_pipeline_graphics(effect.pipeline())?
            .bind_vertex_buffers(0, buffer)?;
        unsafe {
            command_buffer.draw(3, 1, 0, 0)?;
        }

        Ok(())
    }
}
