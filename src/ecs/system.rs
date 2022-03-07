use commands::manager::ChaosCmdManager;
use ecs::manager::ChaosComponentManager;

pub trait ChaosSystem {
    fn initialize(&mut self, component_manager: &mut ChaosComponentManager, cmd_manager: &mut ChaosCmdManager<back::Backend>);
    fn update(&mut self, delta_time: f32, component_manager: &mut ChaosComponentManager, cmd_manager: &mut ChaosCmdManager<back::Backend>) -> Result<(), &'static str>;
}