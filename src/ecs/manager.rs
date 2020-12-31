use std::any::{Any, TypeId};
use std::collections::HashMap;

use ecs::{EntityID, LookupID};
use ecs::component::{Component, RenderComponent};
use ecs::errors::ComponentErrors::{ComponentCastError, ComponentLookupNotFound, ComponentNotFound, EntityNotFound};
use ecs::errors::ComponentErrors;
use ecs::communicator::ChaosCommunicator;
use std::sync::mpsc::Receiver;

pub struct ChaosComponentManager {
    entity_index: HashMap<EntityID, HashMap<TypeId, LookupID>>,
    component_index: HashMap<TypeId, HashMap<EntityID, LookupID>>,
    components: HashMap<LookupID, Box<dyn Any>>,
    current_entity_id: EntityID,
    current_index_id: LookupID,
    communicator: ChaosCommunicator,
    render_component_types: Vec<TypeId>
}

impl ChaosComponentManager {
    pub fn default() -> ChaosComponentManager {
        return ChaosComponentManager::new(5, 5);
    }

    pub fn new(initial_entity_capacity: usize, initial_component_capacity: usize) -> ChaosComponentManager {
        ChaosComponentManager {
            entity_index: HashMap::with_capacity(initial_entity_capacity),
            component_index: HashMap::with_capacity(initial_component_capacity),
            components: HashMap::with_capacity(initial_component_capacity),
            current_entity_id: 0,
            current_index_id: 0,
            communicator: ChaosCommunicator::new(),
            render_component_types: Vec::new()
        }
    }

    pub fn subscribe_to_add<T: Component>(&mut self) -> Receiver<EntityID>{
        self.communicator.register_for_add::<T>()
    }

    pub fn subscribe_to_remove<T: Component>(&mut self) -> Receiver<EntityID>{
        self.communicator.register_for_remove::<T>()
    }

    /// Creates an entity that can be used within the System
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::manager::ChaosComponentManager;
    /// let mut cm = ChaosComponentManager::default();
    /// let entity_id = cm.create_entity();
    /// assert_eq!(0, entity_id);
    /// ```
    pub fn create_entity(&mut self) -> EntityID {
        let id = self.current_entity_id;
        self.current_entity_id += 1;

        self.entity_index.insert(id, HashMap::new());

        id
    }

    /// Adds a component to an already created entity.
    ///
    /// # Arguments
    /// entity_id: the id of the entity
    /// component: The component to add to the entity
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::manager::ChaosComponentManager;
    ///
    /// #[derive(Clone, PartialEq, Debug)]
    /// struct Position {
    ///     x: f32,
    ///     y: f32,
    /// };
    ///
    /// let mut cm = ChaosComponentManager::default();
    ///
    /// let entity_id = cm.create_entity();
    /// assert_eq!(Ok(()), cm.add_component(entity_id, Position{x: 0.0, y: 0.0,}));
    /// ```
    pub fn add_component<T: Component>(&mut self, entity_id: u64, component: T) -> Result<(), ComponentErrors> {
        match self.entity_index.get_mut(&entity_id) {
            Some(e) => {
                let id = self.current_index_id;
                self.current_index_id += 1;

                e.insert(component.type_id(), id);

                self.component_index.entry(TypeId::of::<T>()).
                    or_insert(HashMap::new()).
                    insert(entity_id, id);
                self.components.insert(id, Box::new(component));
                self.communicator.notify_of_add::<T>(entity_id);

                Ok(())
            }
            None => {
                Err(EntityNotFound(entity_id))
            }
        }
    }

    pub fn add_renderable_component<T: RenderComponent<back::Backend>>(&mut self, entity_id: EntityID, component: T) -> Result<(), ComponentErrors> {
        let type_id = TypeId::of::<T>();
        if !self.render_component_types.contains(&type_id) {
            self.render_component_types.push(type_id);
        }
        return self.add_component::<T>(entity_id, component);
    }

    fn lookup_component<T: Component>(&self, lookup_id: &LookupID) -> Result<&T, ComponentErrors> {
        match self.components.get(&lookup_id) {
            Some(component) => {
                Ok(component.downcast_ref::<T>().ok_or(ComponentCastError(TypeId::of::<T>()))?)
            }
            None => {
                Err(ComponentLookupNotFound(lookup_id.clone()))
            }
        }
    }

    fn lookup_component_mut<T: Component>(&mut self, lookup_id: &LookupID) -> Result<&mut T, ComponentErrors> {
        match self.components.get_mut(&lookup_id) {
            Some(component) => {
                Ok(component.downcast_mut::<T>().ok_or(ComponentCastError(TypeId::of::<T>()))?)
            }
            None => {
                Err(ComponentLookupNotFound(lookup_id.clone()))
            }
        }
    }

    fn get_lookup_id<T: Component>(&self, entity_id: EntityID) -> Result<&LookupID, ComponentErrors> {
        match self.entity_index.get(&entity_id) {
            Some(entity) => {
                match entity.get(&TypeId::of::<T>()) {
                    Some(lookup_id) =>
                        {
                            Ok(lookup_id)
                        }
                    None => {
                        Err(ComponentErrors::ComponentNotFound(TypeId::of::<T>()))
                    }
                }
            }
            None => {
                Err(ComponentErrors::EntityNotFound(entity_id))
            }
        }
    }

    /// Gets all components of a type, returning a vector of entity_id and the component instance
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::manager::ChaosComponentManager;
    ///
    /// #[derive(Clone, PartialEq, Debug)]
    /// struct Position {
    ///     x: f32,
    ///     y: f32,
    /// };
    ///
    /// let mut cm = ChaosComponentManager::default();
    /// assert!(cm.get_all_components::<Position>().is_err());
    /// let entity_id = cm.create_entity();
    /// cm.add_component::<Position>(entity_id, Position{x: 0.0, y: 0.0,});
    /// let result = cm.get_all_components::<Position>();
    /// assert!(result.is_ok());
    /// assert!(result.unwrap().len() == 1);
    /// ```
    pub fn get_all_components<T: Component>(&self) -> Result<Vec<(&EntityID, &T)>, ComponentErrors> {
        match self.component_index.get(&TypeId::of::<T>()) {
            Some(components) => {
                let mut result: Vec<(&EntityID, &T)> = Vec::new();
                for (entity_id, lookup_id) in components.iter() {
                    match self.lookup_component::<T>(lookup_id) {
                        Ok(c) => {
                            result.push((entity_id, c));
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    };
                }
                Ok(result)
            }
            None => {
                Err(ComponentNotFound(TypeId::of::<T>()))
            }
        }
    }

    pub fn get_all_components_of_type<T: Component>(&self) -> Result<Vec<&T>, ComponentErrors> {
        match self.component_index.get(&TypeId::of::<T>()) {
            Some(components) => {
                let mut result: Vec<&T> = Vec::new();
                for (_, lookup_id) in components.iter() {
                    match self.lookup_component::<T>(lookup_id) {
                        Ok(c) => {
                            result.push(&c);
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    };
                }
                Ok(result)
            }
            None => {
                Err(ComponentNotFound(TypeId::of::<T>()))
            }
        }
    }

    /// Gets a component of a type for an entity, returning a clone of the entity
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::manager::ChaosComponentManager;
    ///
    /// #[derive(Clone, PartialEq, Debug)]
    /// struct Position {
    ///     x: f32,
    ///     y: f32,
    /// };
    ///
    /// let mut cm = ChaosComponentManager::default();
    /// let entity_id = cm.create_entity();
    /// cm.add_component(entity_id, Position{x: 1234.0, y: 4321.0,});
    /// let entity_component = cm.get_component::<Position>(entity_id).unwrap();
    /// assert_eq!(1234.0, entity_component.x);
    /// assert_eq!(4321.0, entity_component.y);
    ///
    /// ```
    pub fn get_component<T: Component>(&self, entity_id: u64) -> Result<&T, ComponentErrors> {
        return self.lookup_component::<T>(self.get_lookup_id::<T>(entity_id)?);
    }

    /// Gets a component of a type for an entity, returning a clone of the entity
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::manager::ChaosComponentManager;
    ///
    /// #[derive(Clone, PartialEq, Debug)]
    /// struct Position {
    ///     x: f32,
    ///     y: f32,
    /// };
    ///
    /// let mut cm = ChaosComponentManager::default();
    /// let entity_id = cm.create_entity();
    /// cm.add_component(entity_id, Position{x: 1234.0, y: 4321.0,});
    /// let mut entity_component = cm.get_component_mut::<Position>(entity_id).unwrap();
    /// entity_component.x = 10.0;
    /// let changed_component = cm.get_component::<Position>(entity_id).unwrap();
    /// assert_eq!(changed_component.x, 10.0)
    /// ```
    pub fn get_component_mut<T: Component>(&mut self, entity_id: u64) -> Result<&mut T, ComponentErrors> {
        return self.lookup_component_mut::<T>(&self.get_lookup_id::<T>(entity_id)?.clone());
    }

    /// Removes a component from an entity
    pub fn remove_component<T: Component>(&mut self, entity_id: EntityID) -> Result<(), ComponentErrors> {
        let type_id = TypeId::of::<T>();
        let lookup_id = self.entity_index.get_mut(&entity_id).
            ok_or(EntityNotFound(entity_id)).
            unwrap().
            get_mut(&type_id).
            ok_or(ComponentNotFound(type_id)).
            unwrap();

        // remove from the component lookup table
        self.components.remove(lookup_id);

        let component_type = TypeId::of::<T>();

        // remove from the component list for the entity
        let entity_component_lookup = self.entity_index.get_mut(&entity_id).ok_or(EntityNotFound(entity_id))?;
        entity_component_lookup.remove(&component_type);

        // remove from the entity list for the component type
        let component_entity_lookup = self.component_index.get_mut(&component_type).ok_or(ComponentNotFound(component_type))?;
        component_entity_lookup.remove(&entity_id);
        self.communicator.notify_of_removal::<T>(entity_id);

        return Ok(());
    }

    /// Removes an entity
    pub fn remove_entity(&mut self, entity_id: EntityID) -> Result<(), ComponentErrors> {
        let type_lookup = self.entity_index.remove(&entity_id).ok_or(EntityNotFound(entity_id))?;

        for (type_id, _) in type_lookup {
            let lookup_id = self.component_index.get_mut(&type_id).ok_or(ComponentNotFound(type_id))?.remove(&entity_id).ok_or(EntityNotFound(entity_id))?;
            self.components.remove(&lookup_id).ok_or(ComponentLookupNotFound(lookup_id))?;
            self.communicator.notify_of_type_removal(&type_id, entity_id);
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    struct Person {
        _age: u16,
    }

    #[test]
    fn adding_component_to_entity_that_doesnt_exist_not_found_returns_err() {
        let mut cm = ChaosComponentManager::default();
        let entity_id: EntityID = 123;
        assert!(cm.add_component(entity_id, Person { _age: 10 }).is_err())
    }

    #[test]
    fn looking_up_entity_that_doesnt_exist_returns_err() {
        let mut cm = ChaosComponentManager::default();
        let entity_id: EntityID = 123;
        assert!(cm.get_component::<Person>(entity_id).is_err());
        assert!(cm.get_component_mut::<Person>(entity_id).is_err());
    }

    #[test]
    fn looking_up_component_that_doesnt_exist_returns_err() {
        let mut cm = ChaosComponentManager::default();
        let entity_id: EntityID = cm.create_entity();
        assert!(cm.get_component::<Person>(entity_id).is_err());
        assert!(cm.get_component_mut::<Person>(entity_id).is_err());
    }
}
