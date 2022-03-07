use std::sync::{Arc, Mutex};
use chaos_engine::rendering::effect::Effect;
use chaos_engine::rendering::buffer::{Buffer, BufferData};
use chaos_engine::commands::cmd::RenderCmd;
use chaos_engine::rendering::render_context::RenderContext;
use chaos_engine::ChaosBackend;

use std::ops::DerefMut;
use crate::math::Vec2;

// One Quad Vertex, used for well everything...
pub struct QuadVertex {
    pub pos: [f32; 2],
}

impl BufferData for QuadVertex {
    fn layout() -> Vec<u32> {
        return [0 as u32].to_vec();
    }
}

pub const QUAD: [QuadVertex; 3] = [
    QuadVertex{ pos: [-0.05, 0.05] },
    QuadVertex{ pos: [ 0.05, 0.05] },
    QuadVertex{ pos: [ 0.05,-0.05] }
];

#[repr(C)]
pub struct AsteroidPerObjectData {
    pub pos: Vec2
}

impl AsteroidPerObjectData{
    pub fn new() -> AsteroidPerObjectData {
        return AsteroidPerObjectData{
            pos: Vec2{x: 0.0, y:0.0}
        }
    }
}

pub struct AsteroidRenderCommand {
    effect: Arc<Mutex<Effect<ChaosBackend, AsteroidPerObjectData>>>,
    buffer: Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>>
}
impl AsteroidRenderCommand {
    pub fn new(effect: Arc<Mutex<Effect<ChaosBackend, AsteroidPerObjectData>>>, buffer: Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>>) -> AsteroidRenderCommand {
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

