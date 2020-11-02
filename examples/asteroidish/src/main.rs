use chaos_engine::engine::ChaosEngine;
use std::time::Instant;
use chaos_engine::ecs::service::ChaosRenderService;
use chaos_engine::rendering::render_state::RenderState;
use chaos_engine::ecs::manager::ChaosComponentManager;
use chaos_engine::ChaosBackend;
use chaos_engine::rendering::buffer::BufferData;


// game renderer - renders asteroids, ship, explosions and lasers
// asteroid service
// ship service
// lasers fired
// explosions
// split asteroids

// asteroids are 1 quad with a generated texture (showing if there is a "rock" there or not)


///
/// Asteroid vertex information
///
struct AsteroidVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl BufferData for AsteroidVertex {
    fn layout() -> Vec<u32> {
        return [0 as u32, 8 as u32].to_vec();
    }
}


///
/// Ship vertex information
///
struct ShipVertex {
    pos: [f32; 2],
    color: [f32; 3]
}

impl BufferData for ShipVertex {
    fn layout() -> Vec<u32> {
        return [0 as u32, 8 as u32].to_vec();
    }
}
const SHIP_MESH: [ShipVertex; 3] = [
    ShipVertex{ pos: [1.0, 0.5], color: [1.0, 0.0, 1.0] },
    ShipVertex{ pos: [0.5, -0.5], color: [1.0, 1.0, 0.0] },
    ShipVertex{ pos: [0.0, 0.5], color: [1.0, 0.0, 0.0] },
];

const SHIP_MESH_2: [ShipVertex; 3] = [
    ShipVertex{ pos: [0.0, 0.5], color: [0.0, 0.0, 1.0] },
    ShipVertex{ pos: [-0.5, -0.5], color: [0.0, 1.0, 0.0] },
    ShipVertex{ pos: [-1.0, 0.5], color: [1.0, 0.0, 0.0] },
];

///
/// GameRenderer
///
struct GameRenderer {
    // store the asteroid buffer
    // store lasers fired etc
}

impl GameRenderer {
    fn new() -> GameRenderer {
        GameRenderer{}
    }
}

impl ChaosRenderService for GameRenderer {
    fn initialize(&mut self, render_state: &mut RenderState<ChaosBackend>) {
        // initialize the renderers for both the spaceship and the asteroids
        render_state.add_render_pass(&SHIP_MESH, "line.vert".to_string(), "line.frag".to_string());
        render_state.add_render_pass(&SHIP_MESH_2, "line.vert".to_string(), "line.frag".to_string());

    }

    fn update(&mut self, delta_time: f32, component_manager: &mut ChaosComponentManager, render_state: &mut RenderState<ChaosBackend>) {
        // nothing yet
    }
}


fn main() {

    let mut engine = ChaosEngine::new("Asteroidish".to_string(), 1024, 1024).unwrap();
    engine.add_render_service(GameRenderer::new());


    let mut current_time = Instant::now();
    loop {
        if !engine.process_events(){
            break;
        }
        let delta_time = current_time.elapsed().as_secs_f32();
        current_time = Instant::now();
        engine.update(delta_time).unwrap();
        engine.render(delta_time).unwrap();

    }
}