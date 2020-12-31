use commands::manager::CmdManager;

pub trait ChaosSystem {
    fn initialize(&mut self, cmd_manager: &mut CmdManager<back::Backend>);
    fn update(&mut self, delta_time: f32, cmd_manager: &mut CmdManager<back::Backend>);
}