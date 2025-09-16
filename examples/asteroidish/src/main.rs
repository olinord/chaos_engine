// use chaos_engine::commands::cmd::ExitCmd;
use chaos_engine::engine::ChaosEngine;
//use chaos_engine::rendering::effect::CEEffectType;
// use chaos_engine::input::events::KeyCode;
// use chaos_engine::input::manager::ChaosDeviceEventManager;
// use systems::AsteroidRenderSystem;
// use systems::PhysicsSystem;
// use systems::CollisionSystem;
// use crate::systems::AsteroidGenerator;

// mod systems;
// mod components;
// mod commands;
// mod math;

fn main() {
    // let mut input_manager = ChaosDeviceEventManager::new();
    // input_manager.register_multi_key_press::<ExitCmd>(KeyCode::Escape, 3);

    let engine = ChaosEngine::new("Asteroidish", 1024, 1024).unwrap();
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
