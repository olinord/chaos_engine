use commands::cmd::{Cmd, RenderCmd};
use gfx_hal::Backend;
use std::slice::IterMut;

pub struct CmdManager<B: Backend> {
    commands: Vec<Box<dyn Cmd>>,
    render_commands: Vec<Box<dyn RenderCmd<B>>>
}

impl <B:Backend> CmdManager<B> {
    pub fn new() -> CmdManager<B> {
        CmdManager::<B>{ commands: Vec::new(), render_commands: Vec::new() }
    }

    pub fn add_command<T: 'static + Cmd>(&mut self, cmd: Box<T>) {
        self.commands.push(cmd);
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