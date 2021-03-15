use gfx_hal::Backend;
use rendering::render_context::RenderContext;

pub trait Cmd: CmdClone {
    fn execute(&self);
    fn revert(&self) {
        // doesn't have to be implemented, but good to have
    }
}

pub trait CmdClone {
    fn clone_box(&self) -> Box<dyn Cmd>;
}

impl<T> CmdClone for T where T: 'static + Cmd + Clone {
    fn clone_box(&self) -> Box<dyn Cmd> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Cmd> {
    fn clone(&self) -> Box<dyn Cmd> {
        self.clone_box()
    }
}

pub trait RenderCmd<B: Backend> {
    fn render(&mut self, render_context: &mut RenderContext<B>);
}

macro_rules! empty_command {
    ($command_name:ident) => {
        #[derive(Clone)]
        pub struct $command_name {}
        impl $command_name {
            pub fn new() -> $command_name {
                return $command_name{};
            }
        }
        impl Cmd for $command_name {
            fn execute(&self) {}
        }
    }
}

// Common traits
empty_command!(ExitCmd);
