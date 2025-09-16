use std::sync::Arc;

use crate::{ecs::manager::ChaosComponentManager, rendering::rendering_system::ChaosRenderSystem};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    raw_window_handle::HasDisplayHandle,
    window::{WindowAttributes, WindowId},
};
pub struct ChaosEngine {
    component_manager: ChaosComponentManager,
    window: Option<Arc<winit::window::Window>>,
    title: String,
    width: u32,
    height: u32,
}

impl ChaosEngine {
    pub fn new(title: &str, width: u32, height: u32) -> Result<ChaosEngine, &'static str> {
        let component_manager = ChaosComponentManager::new(100, 10);
        Ok(ChaosEngine {
            component_manager,
            window: None,
            title: title.to_string(),
            width,
            height,
        })
    }

    // pub fn initialize(&mut self) -> Result<(), &'static str> {
    //     let rendering_system =
    //         ChaosRenderSystem::new(window_system.get_event_loop(), window_system.get_window());
    //     self.component_manager.add_system(window_system);
    //     self.component_manager.add_system(rendering_system);
    //     Ok(())
    // }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().expect("Couldn't create an eventloop");
        event_loop
            .run_app(&mut self)
            .expect("Failed to run event loop");

        // let render_system = self
        //     .component_manager
        //     .get_system::<ChaosRenderSystem>()
        //     .unwrap();
        // let window_system = self
        //     .component_manager
        //     .get_system::<ChaosWindowSystem>()
        //     .unwrap();

        // let event_loop = window_system.get_event_loop();

        // event_loop.run_app(|event, _, control_flow| match event {
        //     Event::WindowEvent {
        //         event: WindowEvent::CloseRequested,
        //         ..
        //     } => {
        //         *control_flow = winit::event_loop::ControlFlow::Exit;
        //     }
        //     Event::RedrawEventsCleared => {
        //         render_system.render().unwrap();
        //     }
        //     _ => (),
        // });
    }
}

impl ApplicationHandler for ChaosEngine {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let size = PhysicalSize::new(self.width, self.height);
        let window_attributes = WindowAttributes::default()
            .with_title(self.title.clone())
            .with_inner_size(size);

        self.window = Some(Arc::new(
            event_loop.create_window(window_attributes).unwrap(),
        ));

        let rendering_system = ChaosRenderSystem::new(
            &event_loop.display_handle().unwrap(),
            self.window.clone().unwrap(),
        );
        self.component_manager.add_system(rendering_system);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                println!("Redraw requested");
                let c = &mut self.component_manager;
                match c.get_system_mut::<ChaosRenderSystem>() {
                    Some(s) => {
                        s.render();
                    }
                    None => {
                        println!("No rendering system found");
                    }
                };
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                // Draw.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}
