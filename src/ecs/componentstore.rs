use std::{any::Any, collections::HashMap};

use crate::ecs::{EntityID, component::Component};

pub trait ErasedComponentStore {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn remove_entity(&mut self, entity_id: EntityID);
    fn has_entity(&self, entity_id: EntityID) -> bool;
    fn entities(&self) -> Vec<EntityID>;
}

pub struct ComponentStore<T> {
    components: HashMap<EntityID, T>,
}

impl<T> ComponentStore<T> {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    pub fn insert(&mut self, entity: EntityID, component: T) {
        self.components.insert(entity, component);
    }

    pub fn get(&self, entity: EntityID) -> Option<&T> {
        self.components.get(&entity)
    }

    pub fn get_mut(&mut self, entity: EntityID) -> Option<&mut T> {
        self.components.get_mut(&entity)
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.components.values()
    }

    pub fn entity_values(&self) -> impl Iterator<Item = (EntityID, &T)> {
        self.components
            .iter()
            .map(|(id, component)| (*id, component))
    }

    pub fn len(&self) -> usize {
        return self.components.len();
    }
}

impl<T: Component> ErasedComponentStore for ComponentStore<T> {
    fn remove_entity(&mut self, entity: EntityID) {
        self.components.remove(&entity);
    }

    fn has_entity(&self, entity: EntityID) -> bool {
        self.components.contains_key(&entity)
    }

    fn entities(&self) -> Vec<EntityID> {
        self.components.keys().copied().collect()
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }
}
