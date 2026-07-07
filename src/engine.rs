use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::{
    device::system::DeviceEventSystem,
    ecs::{errors::ComponentErrors, world::ChaosWorld},
    rendering::{
        effect_factory::EffectFactory,
        rendering_system::{ChaosRenderContext, ChaosRenderSystem, ChaosRenderableContainer},
    },
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
    device_event_system: DeviceEventSystem,
    window: Option<Arc<winit::window::Window>>,
    pub rendering_system: Option<ChaosRenderSystem>,
    title: String,
    width: u32,
    height: u32,
    directories: HashMap<PathBuf, PathBuf>,
}

impl ChaosEngine {
    pub fn new(title: &str, width: u32, height: u32) -> Result<ChaosEngine, &'static str> {
        let device_event_system = DeviceEventSystem::new();

        Ok(ChaosEngine {
            world: ChaosWorld::new(),
            device_event_system,
            window: None,
            rendering_system: None,
            title: title.to_string(),
            width,
            height,
            directories: HashMap::new(),
        })
    }

    fn initialize_rendering_system(&mut self, event_loop: &ActiveEventLoop) {
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
            &self.directories,
        );
        self.rendering_system = Some(rendering_system);

        // push the directories to the effect factory so we can use shaders
        EffectFactory::instance().load_from_directories(&self.directories, self.render_context());
    }

    pub fn add_directory(&mut self, root: PathBuf, path: PathBuf) {
        self.directories.insert(root, path);
    }

    pub fn device_event_system(&mut self) -> &mut DeviceEventSystem {
        &mut self.device_event_system
    }

    pub fn render_context(&self) -> &Arc<ChaosRenderContext> {
        match &self.rendering_system {
            Some(rendering_system) => rendering_system.render_context(),
            None => panic!("Trying to get render context but rendering system is not initialized"),
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().expect("Couldn't create an eventloop");
        event_loop
            .run_app(&mut self)
            .expect("Failed to run event loop");
    }

    pub fn world_mut(&mut self) -> &mut ChaosWorld {
        &mut self.world
    }

    fn update(&mut self, event: &WindowEvent) -> Result<(), &'static str> {
        for message in self.device_event_system.update(event) {
            if let Err(error) = self.world.try_send_message(message) {
                log::debug!("Input signal was not delivered: {} {:?}", error, event);
            }
        }
        self.world.update()
    }

    fn render(&mut self) -> Result<(), &'static str> {
        let rendering_system = self
            .rendering_system
            .as_mut()
            .ok_or("Rendering system is not initialized")?;

        rendering_system.update(&mut self.world);
        let mut buffer_builder = match rendering_system.start_frame() {
            Some(buffer_builder) => buffer_builder,
            None => return Err("Failed to start frame"),
        };

        let renderables = self
            .world
            .get_all_components_of_type::<ChaosRenderableContainer>();
        let renderables = match renderables {
            Ok(renderables) => renderables,
            Err(ComponentErrors::ComponentNotFound(_)) => Vec::new(),
            Err(err) => panic!("failed to get renderable components: {err:?}"),
        };
        rendering_system.render(renderables, &mut buffer_builder, &self.world);
        rendering_system.end_frame(buffer_builder);
        Ok(())
    }
}

impl ApplicationHandler for ChaosEngine {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.rendering_system.is_none() {
            self.initialize_rendering_system(event_loop);
            self.world.initialize_systems().unwrap_or_else(|err| {
                log::error!("Error initializing systems: {}", err);
            });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.update(&event).unwrap_or_else(|err| {
            log::error!("Error updating world: {}", err);
        });

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(rendering_system) = self.rendering_system.as_mut() {
                    rendering_system.request_resize([size.width, size.height]);
                }
            }
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
                self.render().unwrap_or_else(|err| {
                    log::error!("Error rendering: {}", err);
                });
            }
            _ => (),
        }
    }
}
