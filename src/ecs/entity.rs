use crate::ecs::{EntityID, component::Component, world::ChaosWorld};

pub struct EntityBuilder<'world> {
    world: &'world mut ChaosWorld,
    entity: EntityID,
}

impl<'world> EntityBuilder<'world> {
    pub fn new(entity_id: EntityID, world: &'world mut ChaosWorld) -> EntityBuilder<'world> {
        Self {
            world,
            entity: entity_id,
        }
    }

    pub fn with<T: Component>(self, component: T) -> Self {
        if let Err(e) = self.world.add_component(self.entity, component) {
            panic!("Failed to add component to entity: {:?}", e);
        }
        self
    }

    pub fn build(self) -> EntityID {
        self.entity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestComponent {
        value: i32,
    }

    #[test]
    fn test_entity_builder() {
        let mut world = ChaosWorld::new();
        let entity_id = world.spawn().build();
        assert_eq!(entity_id, 0);
    }

    #[test]
    fn test_entity_builder_with_component() {
        let mut world = ChaosWorld::new();
        let entity_id = world.spawn().with(TestComponent { value: 42 }).build();

        let component = world.get_component::<TestComponent>(entity_id);
        assert!(component.is_ok());
        assert_eq!(component.unwrap().value, 42);
    }
}
