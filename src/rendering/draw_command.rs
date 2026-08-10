use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChaosRenderPhase {
    Opaque,
    Transparent,
    Additive,
}

pub struct ChaosDrawCommand {
    pub render_phase: ChaosRenderPhase,
    pub effect_hash: u64,
    pub draw_function: Box<
        dyn FnOnce(
                &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
            ) -> Result<(), &'static str>
            + Send
            + Sync,
    >,
}

pub type ChaosDrawQueue = Vec<ChaosDrawCommand>;
