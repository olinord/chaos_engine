use commands::manager::ChaosCmdManager;

pub trait ChaosSystem {
    fn initialize(&mut self, cmd_manager: &mut ChaosCmdManager<back::Backend>);
    fn update(&mut self, delta_time: f32, cmd_manager: &mut ChaosCmdManager<back::Backend>);
}