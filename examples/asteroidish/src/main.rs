use chaos_engine::BufferContents;
use chaos_engine::Vertex;
use chaos_engine::device::bindings::ChaosBindingEvent;
use chaos_engine::device::bindings::ChaosButton;
use chaos_engine::device::events::ChaosKeyCode;
use chaos_engine::ecs::component::ChaosComponentManager;
use chaos_engine::ecs::system::ChaosSystem;
use chaos_engine::engine::ChaosEngine;
use chaos_engine::log;
use chaos_engine::logger::ChaosLogger;
use chaos_engine::rendering::buffer::{ChaosBuffer, ChaosBufferMemoryType, ChaosBufferUsage};
use chaos_engine::rendering::effect::ChaosEffect;
use chaos_engine::rendering::effect_factory::{EffectFactory, EffectUsage};
use chaos_engine::rendering::rendering_system::ChaosRenderContext;
use chaos_engine::rendering::rendering_system::{ChaosRenderableContainer, ChaosRenderableTrait};
use chaos_engine::vulkano::ValidationError;
use chaos_engine::vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32B32A32_SFLOAT)]
    color: [f32; 4],
}

struct TriangleRenderable {
    effect: Option<ChaosEffect>,
    buffer: Option<ChaosBuffer>,
}

#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct TriangleMvp {
    projection: [[f32; 3]; 3],
    view: [[f32; 3]; 3],
}

#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct TrianglePushConstants {
    model: [[f32; 3]; 3],
}

const IDENTITY_MAT3: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

impl TriangleRenderable {
    fn new() -> Self {
        Self {
            effect: None,
            buffer: None,
        }
    }
}

impl ChaosRenderableTrait for TriangleRenderable {
    fn initialize(
        &mut self,
        rendering_context: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str> {
        let vertices = vec![
            MyVertex {
                position: [0.0, -0.5],
                color: [1.0, 0.2, 0.1, 1.0],
            },
            MyVertex {
                position: [0.5, 0.5],
                color: [0.1, 0.8, 0.25, 1.0],
            },
            MyVertex {
                position: [-0.5, 0.5],
                color: [0.1, 0.35, 1.0, 1.0],
            },
        ];

        let usage = EffectUsage::new("shaders:/triangle".into());

        let effect = EffectFactory::instance().get_effect::<MyVertex>(&usage, rendering_context);
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
                    projection: IDENTITY_MAT3,
                    view: IDENTITY_MAT3,
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

        Ok(())
    }

    fn add_to_command_buffer(
        &self,
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
        effect.bind_push_constants(
            command_buffer,
            0,
            TrianglePushConstants {
                model: IDENTITY_MAT3,
            },
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

struct TriangleSpawnSystem {
    spawned: bool,
}

impl TriangleSpawnSystem {
    fn new() -> Self {
        Self { spawned: false }
    }
}

impl ChaosSystem for TriangleSpawnSystem {
    fn initialize(
        &mut self,
        _component_manager: &mut ChaosComponentManager,
    ) -> Result<(), &'static str> {
        Ok(())
    }

    fn update(
        &mut self,
        _delta_time: f32,
        component_manager: &mut ChaosComponentManager,
    ) -> Result<(), &'static str> {
        if self.spawned {
            return Ok(());
        }

        let triangle = component_manager.create_entity();
        component_manager
            .add_component(
                triangle,
                ChaosRenderableContainer::new(TriangleRenderable::new()),
            )
            .map_err(|_| "Failed to add triangle renderable")?;
        self.spawned = true;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum KeyEvents {
    Space,
    Other,
}

fn main() {
    log::set_max_level(log::LevelFilter::Debug);
    log::set_logger(&ChaosLogger {}).unwrap();
    let mut engine = ChaosEngine::new("Asteroidish", 1024, 1024).unwrap();
    let shader_root = std::env::current_exe()
        .map_err(|_| "Failed to find current executable")
        .unwrap()
        .parent()
        .ok_or("Failed to find executable directory")
        .unwrap()
        .join("res/shaders");

    engine.add_directory(PathBuf::from("shaders"), shader_root);

    engine.world_mut().add_system(TriangleSpawnSystem::new());

    let binding = ChaosBindingEvent::Held {
        button: ChaosButton::Keyboard(ChaosKeyCode::Space),
        duration: Duration::from_secs(1),
        continue_after_matching: false,
    };

    engine.device_event_system().bind(binding, KeyEvents::Space);

    let binding = ChaosBindingEvent::Held {
        button: ChaosButton::Keyboard(ChaosKeyCode::KeyM),
        duration: Duration::from_secs(1),
        continue_after_matching: true,
    };

    engine.device_event_system().bind(binding, KeyEvents::Other);

    // let effect_builder = engine.create_effect_builder(CEEffectType::Rendering);
    // effect_builder.with_vertex_shader("line.vert".into(), "main".into());
    // effect_builder.with_pixel_shader("line.frag".into(), "main".into());
    engine.run();
    println!("whoop");
    // add_system(AsteroidRenderSystem::new()).
    // add_system(CollisionSystem::new()).
    // add_system(PhysicsSystem::new()).
    // add_system(AsteroidGenerator::new(0.5)).

    // if let Err(r) = result {
    //     println!("Engine shut down due to {}", r);
    // }
}
