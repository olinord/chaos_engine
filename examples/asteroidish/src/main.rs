mod components;
mod renderables;
mod systems;

use chaos_engine::device::bindings::{ChaosBindingEvent, ChaosButton};
use chaos_engine::device::events::ChaosKeyCode;
use chaos_engine::engine::ChaosEngine;
use chaos_engine::log;
use chaos_engine::logger::ChaosLogger;
use std::path::PathBuf;

use crate::systems::ship::ShipEvent;

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

    let binding = ChaosBindingEvent::Held {
        button: ChaosButton::Keyboard(ChaosKeyCode::KeyW),
        duration: std::time::Duration::from_millis(100),
        continuous: true,
    };

    engine
        .device_event_system()
        .bind(binding, ShipEvent::RotateLeft);
    engine
        .world_mut()
        .add_system(systems::transform::TransformSystem::new())
        .add_system(systems::ship::ShipSystem::new());

    engine.run();
}
