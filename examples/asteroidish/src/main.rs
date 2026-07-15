mod components;
mod consts;
mod renderables;
mod systems;

use chaos_engine::device::bindings::{ChaosBindingEvent, ChaosDeviceEventMatcher};
use chaos_engine::device::events::ChaosKeyCode;
use chaos_engine::engine::ChaosEngine;
use chaos_engine::log;
use chaos_engine::logger::ChaosLogger;
use std::path::PathBuf;

use crate::consts::DeviceEvent;
use crate::systems::asteroid::AsteroidSystem;
use crate::systems::camera::CameraSystem;
use crate::systems::impact::ImpactSystem;
use crate::systems::ship::ShipSystem;
use crate::systems::transform::TransformSystem;

use crate::systems::ship::ShipEvent;

fn main() {
    log::set_max_level(log::LevelFilter::Debug);
    log::set_logger(&ChaosLogger {}).unwrap();
    let width = 2048;
    let height = 2048;
    let mut engine = ChaosEngine::new("Asteroidish", width, height).unwrap();
    let shader_root = std::env::current_exe()
        .map_err(|_| "Failed to find current executable")
        .unwrap()
        .parent()
        .ok_or("Failed to find executable directory")
        .unwrap()
        .join("res/shaders");

    engine.add_directory(PathBuf::from("shaders"), shader_root);

    let forward_event = ChaosBindingEvent::keyboard_key_held(
        ChaosKeyCode::KeyW,
        std::time::Duration::from_millis(100),
        true,
    );

    let break_event = ChaosBindingEvent::keyboard_key_held(
        ChaosKeyCode::KeyS,
        std::time::Duration::from_millis(100),
        true,
    );

    let rotate_left_event = ChaosBindingEvent::keyboard_key_held(
        ChaosKeyCode::KeyA,
        std::time::Duration::from_millis(100),
        true,
    );

    let rotate_right_event = ChaosBindingEvent::keyboard_key_held(
        ChaosKeyCode::KeyD,
        std::time::Duration::from_millis(100),
        true,
    );

    engine
        .device_event_system()
        .bind(rotate_left_event, ShipEvent::RotateLeft);
    engine
        .device_event_system()
        .bind(rotate_right_event, ShipEvent::RotateRight);
    engine
        .device_event_system()
        .bind(forward_event, ShipEvent::Thrust);
    engine
        .device_event_system()
        .bind(break_event, ShipEvent::Break);

    engine.device_event_system().bind(
        ChaosBindingEvent::Device(ChaosDeviceEventMatcher::Resized),
        DeviceEvent::Resized,
    );

    engine
        .world_mut()
        .add_system(TransformSystem::new())
        .add_system(ShipSystem::new())
        .add_system(AsteroidSystem::new())
        .add_system(ImpactSystem::new())
        .add_system(CameraSystem::new(width, height));

    engine.run();
}
