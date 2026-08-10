use crate::ecs::system::ChaosSystem;
use crate::ecs::world::ChaosWorld;
use crate::rendering::draw_command::ChaosDrawQueue;
use crate::rendering::renderer::ChaosRenderContext;
use std::sync::Arc;

pub trait ChaosRenderSystem: ChaosSystem {
    fn initialize_rendering(
        &mut self,
        world: &mut ChaosWorld,
        ctx: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str>;

    fn prepare_rendering(
        &mut self,
        world: &mut ChaosWorld,
        draw_queue: &mut ChaosDrawQueue,
        ctx: &Arc<ChaosRenderContext>,
    ) -> Result<(), &'static str>;
}
