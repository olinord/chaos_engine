use chaos_engine::BufferContents;
use chaos_engine::Vertex;
use chaos_engine::ecs::component::ChaosComponentManager;
use chaos_engine::ecs::system::ChaosSystem;
use chaos_engine::engine::ChaosEngine;
use chaos_engine::rendering::buffer::ChaosBuffer;
use chaos_engine::rendering::effect_factory::ShaderCache;
use chaos_engine::rendering::rendering_system::ChaosRenderSystem;

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
    #[format(R32G32B32A32_SFLOAT)]
    color: [f32; 4],
}

struct AsteroidRenderSystem {
    // pipeline: Arc<GraphicsPipeline>,
    buffer: ChaosBuffer<MyVertex>,
}

impl AsteroidRenderSystem {
    fn new() -> AsteroidRenderSystem {
        // fn new(rendering_system: Option<ChaosRenderSystem>) -> AsteroidRenderSystem {
        println!("Creating AsteroidRenderSystem");
        println!("available shaders: {:?}", ShaderCache::instance().list());
        // let pipeline = ShaderCache::instance()
        //     .get_effect::<Vertex>(rendering_system.unwrap(), "asteroid".into())
        //     .unwrap();
        AsteroidRenderSystem {
            // pipeline: pipeline,
            buffer: ChaosBuffer::default(),
        }
    }
}

impl ChaosSystem for AsteroidRenderSystem {
    fn initialize(
        &mut self,
        _component_manager: &mut ChaosComponentManager,
    ) -> Result<(), &'static str> {
        // self.effect
        //     .initialize::<MyVertex>()
        //     .map_err(|_| "Failed to initialize AsteroidRenderSystem effect")?;
        Ok(())
    }
    fn update(
        &mut self,
        _delta_time: f32,
        _component_manager: &mut ChaosComponentManager,
    ) -> Result<(), &'static str> {
        Ok(())
    }
}

fn main() {
    // let mut input_manager = ChaosDeviceEventManager::new();
    // input_manager.register_multi_key_press::<ExitCmd>(KeyCode::Escape, 3);

    let mut engine = ChaosEngine::new("Asteroidish", 1024, 1024).unwrap();
    engine.get_world_mut().add_system(AsteroidRenderSystem::new(
        // engine.rendering_system.unwrap().clone(),
    ));
    engine.run();
    // let effect_builder = engine.create_effect_builder(CEEffectType::Rendering);
    // effect_builder.with_vertex_shader("line.vert".into(), "main".into());
    // effect_builder.with_pixel_shader("line.frag".into(), "main".into());

    println!("whoop");
    // add_system(AsteroidRenderSystem::new()).
    // add_system(CollisionSystem::new()).
    // add_system(PhysicsSystem::new()).
    // add_system(AsteroidGenerator::new(0.5)).
    // set_input_manager(&mut input_manager).run();

    // if let Err(r) = result {
    //     println!("Engine shut down due to {}", r);
    // }
}
