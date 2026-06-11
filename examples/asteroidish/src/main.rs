use chaos_engine::BufferContents;
use chaos_engine::Vertex;
use chaos_engine::ecs::component::ChaosComponentManager;
use chaos_engine::ecs::system::ChaosSystem;
use chaos_engine::engine::ChaosEngine;
use chaos_engine::input;
use chaos_engine::rendering::buffer::{ChaosBuffer, ChaosBufferMemoryType, ChaosBufferUsage};
use chaos_engine::rendering::effect_factory::{EffectFactory, EffectUsage};
use chaos_engine::rendering::rendering_system::{ChaosRenderableContainer, ChaosRenderableTrait};
use chaos_engine::vulkano::ValidationError;
use chaos_engine::vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use chaos_engine::vulkano::device::Device;
use chaos_engine::vulkano::format::Format;
use chaos_engine::vulkano::memory::allocator::{
    FreeListAllocator, GenericMemoryAllocator, StandardMemoryAllocator,
};
use chaos_engine::vulkano::pipeline::GraphicsPipeline;
use chaos_engine::vulkano::pipeline::graphics::viewport::Viewport;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32B32A32_SFLOAT)]
    color: [f32; 4],
}

struct TriangleRenderable {
    pipeline: Mutex<Option<Arc<GraphicsPipeline>>>,
    buffer: Mutex<ChaosBuffer<MyVertex>>,
}

impl TriangleRenderable {
    fn new() -> Self {
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

        Self {
            pipeline: Mutex::new(None),
            buffer: Mutex::new(ChaosBuffer::new(
                "triangle".to_string(),
                vertices,
                ChaosBufferUsage::VertexBuffer,
                ChaosBufferMemoryType::PreferDevice,
            )),
        }
    }
}

impl ChaosRenderableTrait for TriangleRenderable {
    fn initialize(
        &self,
        device: Arc<Device>,
        _memory_allocator: Arc<GenericMemoryAllocator<FreeListAllocator>>,
        viewport: &Viewport,
        color_attachment_format: Format,
    ) -> Result<(), &'static str> {
        let shader_root = std::env::current_exe()
            .map_err(|_| "Failed to find current executable")?
            .parent()
            .ok_or("Failed to find executable directory")?
            .join("res/shaders");

        EffectFactory::instance()
            .load_from_path(&shader_root, &device)
            .map_err(|_| "Failed to load shaders")?;

        println!("loaded shaders: {:?}", EffectFactory::instance().list());

        let usage = EffectUsage {
            path: PathBuf::from(shader_root).join("triangle"),
            viewport: viewport.clone(),
            color_attachment_count: 1,
            color_attachment_format,
        };

        let pipeline = EffectFactory::instance().get_effect::<MyVertex>(&usage, &device);
        if let Err(_) = &pipeline {
            return Err("Failed to create triangle pipeline");
        }
        let pipeline = pipeline.unwrap();

        let standard_allocator = Arc::new(StandardMemoryAllocator::new_default(device));
        self.buffer
            .lock()
            .map_err(|_| "Failed to lock triangle vertex buffer")?
            .initialize(standard_allocator)
            .map_err(|_| "Failed to initialize triangle vertex buffer")?;

        *self
            .pipeline
            .lock()
            .map_err(|_| "Failed to lock triangle pipeline")? = Some(pipeline);

        Ok(())
    }

    fn add_to_command_buffer(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> Result<(), Box<ValidationError>> {
        let pipeline = self.pipeline.lock().unwrap().as_ref().unwrap().clone();
        let buffer = self
            .buffer
            .lock()
            .unwrap()
            .buffer
            .as_ref()
            .unwrap()
            .as_ref()
            .clone();

        command_buffer
            .bind_pipeline_graphics(pipeline)?
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

fn main() {
    let mut engine = ChaosEngine::new("Asteroidish", 1024, 1024).unwrap();
    engine
        .get_world_mut()
        .add_system(TriangleSpawnSystem::new());
    let mut input_manager = engine.get_input_manager();
    input_manager.register_multi_key_press::<ExitCmd>(KeyCode::Escape, 3);

    // let effect_builder = engine.create_effect_builder(CEEffectType::Rendering);
    // effect_builder.with_vertex_shader("line.vert".into(), "main".into());
    // effect_builder.with_pixel_shader("line.frag".into(), "main".into());

    println!("whoop");
    // add_system(AsteroidRenderSystem::new()).
    // add_system(CollisionSystem::new()).
    // add_system(PhysicsSystem::new()).
    // add_system(AsteroidGenerator::new(0.5)).

    // if let Err(r) = result {
    //     println!("Engine shut down due to {}", r);
    // }
}
