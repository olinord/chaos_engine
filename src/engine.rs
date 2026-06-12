use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    ecs::{errors::ComponentErrors, world::ChaosWorld},
    input::manager::ChaosDeviceEventSystem,
    rendering::rendering_system::{ChaosRenderSystem, ChaosRenderableContainer},
};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    raw_window_handle::HasDisplayHandle,
    window::{WindowAttributes, WindowId},
};

pub struct ChaosEngine {
    world: ChaosWorld,
    input_manager: ChaosDeviceEventSystem,
    window: Option<Arc<winit::window::Window>>,
    pub rendering_system: Option<ChaosRenderSystem>,
    title: String,
    width: u32,
    height: u32,
    frame_time: Duration,
}

impl ChaosEngine {
    pub fn new(title: &str, width: u32, height: u32) -> Result<ChaosEngine, &'static str> {
        let input_manager = ChaosDeviceEventSystem::new();

        //let communicator
        Ok(ChaosEngine {
            world: ChaosWorld::new(),
            input_manager,
            window: None,
            rendering_system: None,
            title: title.to_string(),
            width,
            height,
            frame_time: Duration::new(0, 0),
        })
    }

    pub fn get_input_manager(&mut self) -> &mut ChaosDeviceEventSystem {
        &mut self.input_manager
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().expect("Couldn't create an eventloop");
        event_loop
            .run_app(&mut self)
            .expect("Failed to run event loop");
    }

    pub fn get_world_mut(&mut self) -> &mut ChaosWorld {
        &mut self.world
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

        let add_subscription = self.world.subscribe_to_add::<ChaosRenderableContainer>();

        let rendering_system = ChaosRenderSystem::new(
            &event_loop.display_handle().unwrap(),
            self.window.clone().unwrap(),
            add_subscription,
        );
        self.rendering_system = Some(rendering_system);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let start = Instant::now();
        self.input_manager.update_commands(&event);
        // update the systems
        self.world.update(self.frame_time.as_secs_f32()).unwrap();
        self.rendering_system
            .as_mut()
            .unwrap()
            .update(&mut self.world);
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
                let rendering_system = self.rendering_system.as_mut().unwrap();
                let Some(mut buffer_builder) = rendering_system.start_frame() else {
                    return;
                };
                let renderables = self
                    .world
                    .get_all_components_of_type::<ChaosRenderableContainer>();
                let renderables = match renderables {
                    Ok(renderables) => renderables,
                    Err(ComponentErrors::ComponentNotFound(_)) => Vec::new(),
                    Err(err) => panic!("failed to get renderable components: {err:?}"),
                };
                rendering_system.render(renderables, &mut buffer_builder);
                rendering_system.end_frame(buffer_builder);
            }
            _ => (),
        }
        self.frame_time = Instant::duration_since(&start, Instant::now());
    }
}
