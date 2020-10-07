use winit::{dpi::LogicalSize, Event, EventsLoop, WindowEvent};

use ecs::{
    manager::ChaosComponentManager,
    service::{ChaosRenderService, ChaosService},
};
use rendering::render_state::RenderState;

pub struct ChaosEngine<'a> {
    render_state: RenderState<'a, back::Backend>,
    events_loop: EventsLoop,
    chaos_manager: ChaosComponentManager,
    render_services: Vec<Box<dyn ChaosRenderService<'a>>>,
    services: Vec<Box<dyn ChaosService>>,
}

impl<'a> ChaosEngine<'a> {
    pub fn new(name: String, width: u32, height: u32) -> Result<ChaosEngine<'a>, &'static str> {
        let events_loop = EventsLoop::new();

        let wb = winit::WindowBuilder::new()
            .with_min_dimensions(
                LogicalSize::new(64.0, 64.0)
            )
            .with_max_dimensions(
                LogicalSize::new(width.into(), height.into())
            )
            .with_title(name.clone());
        let window = wb.build(&events_loop).unwrap();

        let render_state = RenderState::<back::Backend>::new(window).unwrap();

        return Ok(ChaosEngine {
            events_loop,
            render_state,
            chaos_manager: ChaosComponentManager::new(100, 10),
            render_services: Vec::new(),
            services: Vec::new(),
        })
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), &'static str> {
        for service in self.services.iter_mut() {
            service.update(delta_time, &mut self.chaos_manager);
        }
        for service in self.render_services.iter_mut() {
            service.update(delta_time, &mut self.chaos_manager, &mut self.render_state);
        }
        Ok(())
    }

    pub fn render(&mut self, delta_time: f32) -> Result<(), &'static str> {
        self.render_state.render(delta_time)
    }

    pub fn process_events(&mut self) -> bool {
        let mut continue_rendering = true;
        self.events_loop.poll_events(|event| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => continue_rendering = false,
            _ => continue_rendering = true,
        });
        return continue_rendering;
    }

    /// Adds a service to the manager and initializes it
    pub fn add_service<CS: ChaosService>(&mut self, service: CS) {
        self.services.push(Box::new(service));
        self.services.last_mut().unwrap().initialize();
    }

    pub fn add_render_service<CRS: ChaosRenderService<'a>>(&mut self, render_service: CRS) {
        self.render_services.push(Box::new(render_service));
        self.render_services.last_mut().unwrap().initialize(&mut self.render_state);
    }

    pub fn get_render_state(&mut self) -> &mut RenderState<'a, back::Backend> {
        return &mut self.render_state;
    }
}

