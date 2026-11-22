use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::{
    device::system::DeviceEventSystem,
    ecs::world::ChaosWorld,
    errors::EngineError,
    rendering::{
        context::ChaosRenderContext, effect_factory::EffectFactory, errors::error_chain,
        renderer::ChaosRenderer,
    },
    trace_zone,
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
    pub rendering_system: Option<ChaosRenderer>,
    title: String,
    width: u32,
    height: u32,
    directories: HashMap<PathBuf, PathBuf>,
    // Set when a fatal error stops the event loop, so `run` can return it.
    fatal_error: Option<EngineError>,
}

impl ChaosEngine {
    pub fn new(title: &str, width: u32, height: u32) -> Result<ChaosEngine, EngineError> {
        crate::telemetry::init();
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
            fatal_error: None,
        })
    }

    fn initialize_rendering_system(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), EngineError> {
        let size = PhysicalSize::new(self.width, self.height);
        let window_attributes = WindowAttributes::default()
            .with_title(self.title.clone())
            .with_inner_size(size);

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        self.window = Some(window.clone());

        let rendering_system =
            ChaosRenderer::new(&event_loop.display_handle()?, window, &self.directories)?;
        self.rendering_system = Some(rendering_system);

        // push the directories to the effect factory so we can use shaders
        EffectFactory::instance().load_from_directories(&self.directories, self.render_context());
        Ok(())
    }

    /// The one place that decides what to do with an error that stops the engine:
    /// log it with its full cause chain, remember it for `run`, and exit the event loop.
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: EngineError) {
        log::error!("Fatal error: {}", error_chain(&error));
        // Events can still arrive before the loop exits; keep the first error, it's the cause.
        if self.fatal_error.is_none() {
            self.fatal_error = Some(error);
        }
        event_loop.exit();
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

    /// Runs the engine until the window closes. Returns the fatal error that stopped it,
    /// if any.
    pub fn run(mut self) -> Result<(), EngineError> {
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        match self.fatal_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn world_mut(&mut self) -> &mut ChaosWorld {
        &mut self.world
    }

    fn forward_device_event(&mut self, event: &WindowEvent) {
        trace_zone!("engine forward device event");
        for message in self.device_event_system.update(event) {
            if let Err(error) = self.world.try_send_message(message) {
                log::debug!("Input signal was not delivered: {} {:?}", error, event);
            }
        }
    }

    fn update(&mut self) -> Result<(), EngineError> {
        trace_zone!("engine update");
        self.world.update().map_err(EngineError::World)
    }

    fn render(&mut self) -> Result<(), EngineError> {
        trace_zone!("engine render");
        // Window events only arrive once `resumed` has created the window and renderer.
        let Some(rendering_system) = self.rendering_system.as_mut() else {
            return Ok(());
        };

        // `None` means the swapchain is being recreated; skip this frame.
        let Some(mut buffer) = rendering_system.start_frame()? else {
            return Ok(());
        };

        let draw_queue = self.world.get_draw_queue(rendering_system.render_context());

        rendering_system.render(draw_queue, &mut buffer)?;
        rendering_system.end_frame(buffer)?;
        Ok(())
    }
}

impl ApplicationHandler for ChaosEngine {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.rendering_system.is_none() {
            if let Err(error) = self.initialize_rendering_system(event_loop) {
                self.fail(event_loop, error);
                return;
            }
            self.world.initialize_systems().unwrap_or_else(|err| {
                log::error!("Error initializing systems: {}", err);
            });
            let render_context = self.render_context().clone();
            self.world
                .initialize_render_systems(&render_context)
                .unwrap_or_else(|err| {
                    log::error!("Error initializing render systems: {}", err);
                });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.forward_device_event(&event);

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
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                // A failing game system is logged but doesn't stop the engine.
                if let Err(error) = self.update() {
                    log::error!("{}", error_chain(&error));
                }
                // The renderer handles recoverable conditions itself (an out-of-date
                // swapchain just skips the frame), so any error that gets here is fatal.
                if let Err(error) = self.render() {
                    self.fail(event_loop, error);
                }
            }
            _ => (),
        }
    }
}
