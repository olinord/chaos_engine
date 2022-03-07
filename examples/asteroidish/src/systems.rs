use chaos_engine::ecs::system::ChaosSystem;
use chaos_engine::commands::manager::ChaosCmdManager;
use chaos_engine::ecs::manager::ChaosComponentManager;
use chaos_engine::ChaosBackend;
use std::sync::{Arc, Mutex};
use chaos_engine::rendering::effect::Effect;
use chaos_engine::rendering::buffer::{Buffer, BufferData};
use crate::commands::{QuadVertex, AsteroidRenderCommand, QUAD, AsteroidPerObjectData};
use core::mem;
use std::collections::hash_map;
use std::ops::Deref;
use crate::components::{CollisionComponent, PhysicsComponent, AsteroidRenderComponent, VoxelData};
use crate::math::Vec2;
use chaos_engine::ecs::EntityID;
use std::sync::mpsc::Receiver;
use rand::Rng;

extern crate array_tool;


///
/// Collision system that handles collision between two collidables
///
/// Components used:
/// CollisionComponent
///
/// Events Fired:
/// CollisionOccurred(collisionPoint)
///

pub struct CollisionSystem{
}

impl CollisionSystem{
    pub fn new() -> CollisionSystem {
        return CollisionSystem{
        }
    }
}

impl ChaosSystem for CollisionSystem {
    fn initialize(&mut self, _component_manager: &mut ChaosComponentManager, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) {

    }

    fn update(&mut self, _delta_time: f32, component_manager: &mut ChaosComponentManager, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) -> Result<(), &'static str>{

        // get all physics components
        // let collision_components = component_manager.get_all_entities_with_component::<CollisionComponent>();
        //
        // if collision_components.len() == 0 {
        //     return Ok(());
        // }
        // let mut physics_components: Vec<Option<&EntityID>> = collision_components.iter().map(| entity_id | {
        //     let c = component_manager.get_component::<PhysicsComponent>(*entity_id);
        //     if c.is_ok() {
        //         Some(entity_id)
        //     }
        //     else{
        //         None
        //     }
        // }).collect();
        //
        // for i in 0..(collision_components.len()-1){
        //
        //     if physics_components[i].is_none(){
        //         continue;
        //     }
        //     let i_entity = collision_components[i];
        //     let i_collision = component_manager.get_component::<CollisionComponent>(i_entity)?;
        //     let ith_collision_radius = i_collision.get_bounding_radius();
        //     let i_physics = component_manager.get_component_mut::<PhysicsComponent>(i_entity)?;
        //     let i_pos = i_physics.get_position();
        //
        //     for j in i+1..collision_components.len(){
        //
        //         if physics_components[j].is_none(){
        //             continue;
        //         }
        //         let j_entity = collision_components[i];
        //         let j_collision = {
        //             component_manager.get_component_mut::<CollisionComponent>(j_entity)?;
        //         };
        //         let jth_collision_radius = j_collision.get_bounding_radius();
        //         let j_physics = component_manager.get_component_mut::<PhysicsComponent>(j_entity)?;
        //         let j_pos = j_physics.get_position();
        //
        //         if Vec2::dist(i_pos, j_pos) < ith_collision_radius + jth_collision_radius {
        //             // calculate the new velocity
        //             i_physics.collide(j_physics);
        //         }
        //     }
        // }
        return Ok(());
    }
}


pub struct PhysicsSystem {
    // gravity: f32
}

impl PhysicsSystem{
    pub fn new() -> Self {
        return PhysicsSystem{
            // gravity: 0.0
        }
    }
}

impl ChaosSystem for PhysicsSystem {
    fn initialize(&mut self, _component_manager: &mut ChaosComponentManager, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) {

    }

    fn update(&mut self, delta_time: f32, component_manager: &mut ChaosComponentManager, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) -> Result<(), &'static str>{
        // let entities = component_manager.get_all_entities_with_component::<PhysicsComponent>();
        //
        // for entity_id in entities {
        //     let check = component_manager.get_component_mut::<PhysicsComponent>(entity_id);
        //     if let Ok(component) = check {
        //         component.update(delta_time);
        //     }
        // }
        return Ok(());
    }
}


///
/// Asteroid render system
/// Renders all asteroids in the game at once

pub struct AsteroidRenderSystem {
    renderables: Vec<(EntityID, Arc<Mutex<Effect<ChaosBackend, AsteroidPerObjectData>>>, Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>>)>,
    asteroid_renderable_component_added: Option<Receiver<EntityID>>,
    asteroid_renderable_component_removed: Option<Receiver<EntityID>>
}

impl AsteroidRenderSystem {
    pub fn new()-> AsteroidRenderSystem  {
        AsteroidRenderSystem{
            renderables: Vec::new(),
            asteroid_renderable_component_added: None,
            asteroid_renderable_component_removed: None
        }
    }

    fn make_asteroid(&mut self, entity_id: EntityID) {
        let stride = mem::size_of::<QuadVertex>();

        let effect = Effect::<ChaosBackend, AsteroidPerObjectData>::new_vs_ps("line.vert".to_string(), "line.frag".to_string(), stride , QuadVertex::layout());
        let buffer = Buffer::<ChaosBackend, QuadVertex>::new(Vec::from(QUAD));

        self.renderables.push((entity_id, Arc::new( Mutex::new(effect)), Arc::new(Mutex::new(buffer))));
    }

    fn remove_asteroid(&mut self, entity_id: EntityID) {
        self.renderables.retain(| (e_id, _, __) | *e_id == entity_id);
    }
}

///
/// Rendering system that renders all asteroids
///
/// components needed:
/// AsteroidRenderComponent (for knowing what is an asteroid)
/// PhysicsComponent: (position)
///
/// Subscribed to:
/// AsteroidRenderComponent - ComponentAdded/ComponentRemoved (
impl ChaosSystem for AsteroidRenderSystem {
    fn initialize(&mut self, component_manager: &mut ChaosComponentManager, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) {
        self.asteroid_renderable_component_added = Some(component_manager.subscribe_to_add::<AsteroidRenderComponent>());
        self.asteroid_renderable_component_removed = Some(component_manager.subscribe_to_remove::<AsteroidRenderComponent>());
    }

    fn update(&mut self, _delta_time: f32, component_manager: &mut ChaosComponentManager, cmd_manager: &mut ChaosCmdManager<ChaosBackend>) -> Result<(), &'static str>{
        // find newly added asteroids or removed asteroids
        for entity_id in self.asteroid_renderable_component_removed.as_mut().unwrap().try_recv() {
            self.remove_asteroid(entity_id);
        }

        for entity_id in self.asteroid_renderable_component_added.as_mut().unwrap().try_recv() {
            self.make_asteroid(entity_id);
        }

        for (entity, effect, buffer) in &self.renderables {
            let physics_component = component_manager.get_component::<PhysicsComponent>(*entity);
            let asteroid_component = component_manager.get_component::<AsteroidRenderComponent>(*entity).unwrap();
            let mut constant = AsteroidPerObjectData::new();

            if let Ok(pc) = physics_component {
                constant.pos = pc.get_position().clone();
            }

            let mut e = effect.lock().unwrap();
            e.set_push_constant(constant);

            cmd_manager.add_render_command(Box::new(AsteroidRenderCommand::new(
                effect.clone(),
                asteroid_component.get_vertex_buffer())));
        }
        Ok(())
    }
}

pub struct AsteroidGenerator {
    sec_since_generation: f32,
    interval_in_sec: f32
}

impl AsteroidGenerator {
    pub fn new(interval_in_sec: f32) -> Self {
        return AsteroidGenerator{
            sec_since_generation: 0.0,
            interval_in_sec
        }
    }
}

impl ChaosSystem for AsteroidGenerator {
    fn initialize(&mut self, _component_manager: &mut ChaosComponentManager, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) {

    }


    fn update(&mut self, delta_time: f32, component_manager: &mut ChaosComponentManager, _cmd_manager: &mut ChaosCmdManager<ChaosBackend>) -> Result<(), &'static str> {
        self.sec_since_generation += delta_time;
        if self.sec_since_generation >= self.interval_in_sec {
            self.sec_since_generation = self.sec_since_generation % self.interval_in_sec;
            let new_entity = component_manager.create_entity();

            let mut rng = rand::thread_rng();

            // make the physics component
            if component_manager.add_component(new_entity, PhysicsComponent::new(
                Vec2{x: rng.gen_range(-1.0..1.0), y: rng.gen_range(-1.0..1.0)},
                Vec2{x: rng.gen_range(-0.25..0.25), y:rng.gen_range(-0.25..0.25)},
                rng.gen_range(0.1..1.0))
            ).is_err()
            {
                return Err("Error updating AsteroidGenerator");
            }

            // make the render component
            let width = rng.gen_range(10..30) as usize;
            let height = rng.gen_range(10..30) as usize;

            if component_manager.add_component(new_entity,
                                            AsteroidRenderComponent::new(VoxelData::generate_random_asteroid(width, height))).is_err() {
                return Err("Error updating Asteroid Generator");
            }

            // make the collision component
            if component_manager.add_component(new_entity,
            CollisionComponent::new((width as f32).max(height as f32))).is_err() {
                return Err("Error creating collision component");
            }
        }
        Ok(())
    }
}

