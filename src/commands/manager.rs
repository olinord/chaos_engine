use commands::cmd::RenderCmd;
use gfx_hal::Backend;
use std::slice::IterMut;
use std::any::{TypeId, Any};

pub struct ChaosCmdManager<B: Backend> {
    commands: Vec<TypeId>,
    render_commands: Vec<Box<dyn RenderCmd<B>>> // This is a bit strange...
}

impl <B:Backend> ChaosCmdManager<B> {
    pub fn new() -> ChaosCmdManager<B> {
        ChaosCmdManager::<B>{ commands: Vec::new(), render_commands: Vec::new() }
    }

    pub fn add_command<T: Any>(&mut self) {
        self.commands.push(TypeId::of::<T>());
    }

    pub fn add_command_raw(&mut self, type_id: TypeId) {
        self.commands.push(type_id);
    }

    pub fn has_command<T: Any>(&self) -> bool {
        return self.commands.contains(&TypeId::of::<T>());
    }

    pub fn add_render_command<T: 'static + RenderCmd<B>>(&mut self, render_cmd: Box<T>) {
        self.render_commands.push(render_cmd);
    }

    pub fn clear_render_commands(&mut self) {
        self.render_commands.clear();
    }

    pub fn get_render_commands(&mut self) -> IterMut<'_, Box<dyn RenderCmd<B>>> {
        return self.render_commands.iter_mut();
    }

    pub fn clear_commands(&mut self) {
        self.commands.clear();
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use commands::cmd::Cmd;
    //
    // #[derive(Debug)]
    // struct TestCmd {
    //     pub identifier: u8
    // }
    //
    // impl Cmd for TestCmd {
    //     fn execute(&self) {
    //     }
    // }
    //
    // #[test]
    // fn registered_command_can_be_retrieved() {
    //     let mut cmd_man = ChaosCmdManager::<back::Backend>::new();
    //     let cmd_box = Box::new(TestCmd{identifier:128});
    //     cmd_man.add_command(cmd_box);
    //     let retreived_command = cmd_man.get_command::<TestCmd>();
    //     assert!(retreived_command.is_some());
    //     assert_eq!(retreived_command.unwrap().identifier, 128);
    // }
    //
    // #[test]
    // fn clear_commands_will_clear_all_commands() {
    //     let mut cmd_man = ChaosCmdManager::<back::Backend>::new();
    //     let cmd_box = Box::new(TestCmd{identifier:128});
    //     cmd_man.add_command(cmd_box);
    //     cmd_man.clear_commands();
    //     assert!(cmd_man.get_command::<TestCmd>().is_none());
    // }
}