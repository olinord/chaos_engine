use chaos_engine::engine::ChaosEngine;
use chaos_engine::ecs::system::ChaosSystem;
use chaos_engine::commands::manager::ChaosCmdManager;
use chaos_engine::rendering::effect::Effect;
use chaos_engine::rendering::buffer::{BufferData, Buffer};
use chaos_engine::ChaosBackend;
use chaos_engine::commands::cmd::{RenderCmd, ExitCmd};
use chaos_engine::rendering::render_context::RenderContext;
use std::sync::{Arc, Mutex};
use std::ops::DerefMut;
use std::mem;
use chaos_engine::input::manager::ChaosDeviceEventManager;
use chaos_engine::input::events::KeyCode;

pub struct QuadVertex {
    #[allow(unused)]
    pos: [f32; 2],
}

impl BufferData for QuadVertex {
    fn layout() -> Vec<u32> {
        return [0 as u32].to_vec();
    }
}
const QUAD: [QuadVertex; 6] = [
    QuadVertex{ pos: [-0.5, 0.33] },
    QuadVertex{ pos: [ 0.5, 0.33] },
    QuadVertex{ pos: [ 0.5,-0.33] },

    QuadVertex{ pos: [-0.5, 0.33] },
    QuadVertex{ pos: [ 0.5,-0.33] },
    QuadVertex{ pos: [-0.5,-0.33] }
];

pub struct AsteroidSystem {
    effect: Arc<Mutex<Effect<ChaosBackend>>>,
    buffer: Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>>,
}

impl AsteroidSystem {
    pub fn new()-> AsteroidSystem  {
        let stride = mem::size_of::<QuadVertex>();
        let effect = Arc::new(Mutex::new(Effect::<ChaosBackend>::new_vs_ps("line.vert".to_string(), "line.frag".to_string(), stride , QuadVertex::layout())));
        let buffer = Arc::new(Mutex::new(Buffer::<ChaosBackend, QuadVertex>::new(Vec::from(QUAD))));
        AsteroidSystem{
            effect,
            buffer,
        }
    }
}

impl ChaosSystem for AsteroidSystem {
    fn initialize(&mut self, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) {
    }

    fn update(&mut self, _delta_time: f32, cmd_manager: &mut ChaosCmdManager<ChaosBackend>) {
        cmd_manager.add_render_command(Box::new(AsteroidRenderCommand::new(self.effect.clone(), self.buffer.clone())));
    }
}

pub struct AsteroidRenderCommand {
    effect: Arc<Mutex<Effect<ChaosBackend>>>,
    buffer: Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>>
}

impl AsteroidRenderCommand {
    pub fn new(effect: Arc<Mutex<Effect<ChaosBackend>>>, buffer: Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>>) -> AsteroidRenderCommand {
        return AsteroidRenderCommand{
            effect,
            buffer
        }
    }
}

impl RenderCmd<ChaosBackend> for AsteroidRenderCommand {
    fn render(&mut self, render_context: &mut RenderContext<ChaosBackend>) {
        let mut b = self.buffer.lock().unwrap();
        render_context.prepare_effect(self.effect.lock().unwrap().deref_mut());
        render_context.prepare_buffer(b.deref_mut());
        render_context.draw(0..b.get_length(), 0..1);
    }
}



fn main() {

    let mut input_manager = ChaosDeviceEventManager::new();
    input_manager.register_multi_key_press::<ExitCmd>(KeyCode::Escape, 3);

    let result = ChaosEngine::new("Asteroidish".to_string(), 1024, 1024).unwrap().
        add_system(AsteroidSystem::new()).
        set_input_manager(&mut input_manager).run();

    if let Err(r) = result {
        println!("Engine shut down due to {}", r);
    }
}