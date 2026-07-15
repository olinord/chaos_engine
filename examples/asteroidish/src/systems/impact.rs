use chaos_engine::ecs::{system::ChaosSystem, world::ChaosWorld};
use chaos_engine::log;
use chaos_engine::math::shape::triangle::Triangle2D;

use crate::{
    components::{shape::ShapeComponent, transform::TransformComponent},
    consts::SpecializedEntities,
};

pub struct ImpactSystem {}

impl ImpactSystem {
    pub fn new() -> Self {
        Self {}
    }
}

impl ChaosSystem for ImpactSystem {
    fn initialize(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        // Locate the ship. If it has already been destroyed there is nothing to do.
        let ship_entity = match world.get_specialized_entity(SpecializedEntities::Ship) {
            Some(entity) => entity,
            None => return Ok(()),
        };

        // Snapshot the ship's collision data so we can release the world borrow
        // before running the query below.
        let (ship_position, ship_radius, ship_triangles) = {
            let transform = world
                .get_component::<TransformComponent>(ship_entity)
                .ok_or("Ship missing TransformComponent")?;
            let shape = world
                .get_component::<ShapeComponent>(ship_entity)
                .ok_or("Ship missing ShapeComponent")?;
            let triangles: Vec<Triangle2D> = shape
                .shape
                .iter()
                .map(|tri| {
                    let matrix = transform.as_mat3();
                    tri * matrix
                })
                .collect();
            (transform.position, shape.bounding_radius, triangles)
        };

        // Broad phase: bounding-sphere overlap. Narrow phase: triangle-vs-triangle SAT.
        let mut query = world
            .query::<(&TransformComponent, &ShapeComponent)>()
            .map_err(|_| "Failed to query for collidable entities")?;

        let mut collided = false;
        'outer: for (entity, (transform, shape)) in query.iter_mut() {
            if entity == ship_entity {
                continue;
            }

            // Broad phase.
            let combined = ship_radius + shape.bounding_radius;
            let distance_sq = (transform.position - ship_position).length_squared();
            if distance_sq > combined * combined {
                continue;
            }

            // Narrow phase: check every ship triangle against every asteroid triangle.
            for ship_tri in &ship_triangles {
                for asteroid_tri in &shape.shape {
                    let asteroid_tri = asteroid_tri * transform.as_mat3();
                    if Triangle2D::intersect(ship_tri, &asteroid_tri) {
                        collided = true;
                        break 'outer;
                    }
                }
            }
        }

        // Drop the query borrow before mutating the world.
        drop(query);

        if collided {
            log::info!("Ship destroyed by asteroid impact");
            world.despawn(ship_entity);
            world.unregister_specialized_entity(SpecializedEntities::Ship);
        }

        Ok(())
    }
}
