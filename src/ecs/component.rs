use std::{
    any::{Any, TypeId, type_name},
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chaos_communicator::{
    communicator::{ChaosCommunicator, ChaosReceiver},
    message::ChaosMessageBuilder,
};

use crate::ecs::{EntityID, LookupID, errors::ComponentErrors};

pub trait Component: Any {}
impl<T: Any> Component for T {}

pub struct ChaosComponentManager {
    entity_index: HashMap<EntityID, HashMap<TypeId, LookupID>>,
    component_index: HashMap<TypeId, HashMap<EntityID, LookupID>>,
    components: HashMap<LookupID, Box<dyn Any>>,
    current_entity_id: EntityID,
    current_index_id: LookupID,
    communicator: Arc<Mutex<ChaosCommunicator>>,
}

impl Default for ChaosComponentManager {
    fn default() -> Self {
        ChaosComponentManager::new(5, 5, Arc::new(Mutex::new(ChaosCommunicator::new())))
    }
}

impl ChaosComponentManager {
    pub fn new(
        initial_entity_capacity: usize,
        initial_component_capacity: usize,
        communicator: Arc<Mutex<ChaosCommunicator>>,
    ) -> ChaosComponentManager {
        ChaosComponentManager {
            entity_index: HashMap::with_capacity(initial_entity_capacity),
            component_index: HashMap::with_capacity(initial_component_capacity),
            components: HashMap::with_capacity(initial_component_capacity),
            current_entity_id: 0,
            current_index_id: 0,
            communicator,
        }
    }

    pub fn subscribe_to_add<T: Component>(&mut self) -> ChaosReceiver {
        let mut guard = self.communicator.lock();
        match guard {
            Ok(ref mut comm) => comm.register_for(format!("add_{}", type_name::<T>())),
            Err(_) => panic!("Failed to acquire communicator lock"),
        }
    }

    pub fn subscribe_to_remove<T: Component>(&mut self) -> ChaosReceiver {
        let mut guard = self.communicator.lock();
        match guard {
            Ok(ref mut comm) => comm.register_for(format!("remove_{}", type_name::<T>())),
            Err(_) => panic!("Failed to acquire communicator lock"),
        }
    }
    /// Creates an entity that can be used within the System
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::component::ChaosComponentManager;
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
    /// use chaos_engine::ecs::component::ChaosComponentManager;
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
    pub fn add_component<T: Component>(
        &mut self,
        entity_id: u64,
        component: T,
    ) -> Result<(), ComponentErrors> {
        match self.entity_index.get_mut(&entity_id) {
            Some(e) => {
                let id = self.current_index_id;
                self.current_index_id += 1;

                if let Some(previous_lookup_id) = e.insert(component.type_id(), id) {
                    self.components.remove(&previous_lookup_id);
                }

                self.component_index
                    .entry(TypeId::of::<T>())
                    .or_default()
                    .insert(entity_id, id)
                    .map(|previous_lookup_id| self.components.remove(&previous_lookup_id));
                self.components.insert(id, Box::new(component));

                let mut guard = self.communicator.lock();
                match guard {
                    Ok(ref mut comm) => {
                        let _ = comm.send_message(
                            ChaosMessageBuilder::new()
                                .with_param("entity_id", entity_id)
                                .build_for_event(format!("add_{}", type_name::<T>())),
                        );
                    }
                    Err(_) => panic!("Failed to acquire communicator lock"),
                };

                Ok(())
            }
            None => Err(ComponentErrors::EntityNotFound(entity_id)),
        }
    }

    fn lookup_component<T: Component>(&self, lookup_id: &LookupID) -> Result<&T, ComponentErrors> {
        match self.components.get(lookup_id) {
            Some(component) => Ok(component
                .downcast_ref::<T>()
                .ok_or(ComponentErrors::ComponentCastError(TypeId::of::<T>()))?),
            None => Err(ComponentErrors::ComponentLookupNotFound(*lookup_id)),
        }
    }

    fn lookup_component_mut<T: Component>(
        &mut self,
        lookup_id: &LookupID,
    ) -> Result<&mut T, ComponentErrors> {
        match self.components.get_mut(lookup_id) {
            Some(component) => Ok(component
                .downcast_mut::<T>()
                .ok_or(ComponentErrors::ComponentCastError(TypeId::of::<T>()))?),
            None => Err(ComponentErrors::ComponentLookupNotFound(*lookup_id)),
        }
    }

    fn get_lookup_id<T: Component>(
        &self,
        entity_id: EntityID,
    ) -> Result<&LookupID, ComponentErrors> {
        match self.entity_index.get(&entity_id) {
            Some(entity) => match entity.get(&TypeId::of::<T>()) {
                Some(lookup_id) => Ok(lookup_id),
                None => Err(ComponentErrors::ComponentNotFound(TypeId::of::<T>())),
            },
            None => Err(ComponentErrors::EntityNotFound(entity_id)),
        }
    }

    /// Gets all components of a type, returning a vector of entity_id and the component instance
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::component::ChaosComponentManager;
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
    pub fn get_all_components<T: Component>(
        &self,
    ) -> Result<Vec<(&EntityID, &T)>, ComponentErrors> {
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
            None => Err(ComponentErrors::ComponentNotFound(TypeId::of::<T>())),
        }
    }

    pub fn get_all_components_of_type<T: Component>(&self) -> Result<Vec<&T>, ComponentErrors> {
        match self.component_index.get(&TypeId::of::<T>()) {
            Some(components) => {
                let mut result: Vec<&T> = Vec::new();
                for (_, lookup_id) in components.iter() {
                    match self.lookup_component::<T>(lookup_id) {
                        Ok(c) => {
                            result.push(c);
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    };
                }
                Ok(result)
            }
            None => Err(ComponentErrors::ComponentNotFound(TypeId::of::<T>())),
        }
    }

    /// Gets a component of a type for an entity, returning a clone of the entity
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::component::ChaosComponentManager;
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
        self.lookup_component::<T>(self.get_lookup_id::<T>(entity_id)?)
    }

    /// Gets a component of a type for an entity, returning a clone of the entity
    ///
    /// # Examples
    /// ```
    /// use chaos_engine::ecs::component::ChaosComponentManager;
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
    pub fn get_component_mut<T: Component>(
        &mut self,
        entity_id: u64,
    ) -> Result<&mut T, ComponentErrors> {
        let lookup_id = *self.get_lookup_id::<T>(entity_id)?;
        self.lookup_component_mut::<T>(&lookup_id)
    }

    /// Removes a component from an entity
    pub fn remove_component<T: Component>(
        &mut self,
        entity_id: EntityID,
    ) -> Result<(), ComponentErrors> {
        let type_id = TypeId::of::<T>();
        let lookup_id = *self
            .entity_index
            .get_mut(&entity_id)
            .ok_or(ComponentErrors::EntityNotFound(entity_id))?
            .get_mut(&type_id)
            .ok_or(ComponentErrors::ComponentNotFound(type_id))?;

        // remove from the component lookup table
        self.components
            .remove(&lookup_id)
            .ok_or(ComponentErrors::ComponentLookupNotFound(lookup_id))?;

        let component_type = TypeId::of::<T>();

        // remove from the component list for the entity
        let entity_component_lookup = self
            .entity_index
            .get_mut(&entity_id)
            .ok_or(ComponentErrors::EntityNotFound(entity_id))?;
        entity_component_lookup.remove(&component_type);

        // remove from the entity list for the component type
        let component_entity_lookup = self
            .component_index
            .get_mut(&component_type)
            .ok_or(ComponentErrors::ComponentNotFound(component_type))?;
        component_entity_lookup.remove(&entity_id);

        let mut guard = self.communicator.lock();
        match guard {
            Ok(ref mut comm) => {
                let _ = comm.send_message(
                    ChaosMessageBuilder::new()
                        .with_param("entity_id", entity_id)
                        .build_for_event(format!("remove_{}", type_name::<T>())),
                );
            }
            Err(_) => panic!("Failed to acquire communicator lock"),
        };
        Ok(())
    }

    /// Removes an entity
    pub fn remove_entity(&mut self, entity_id: EntityID) -> Result<(), ComponentErrors> {
        let type_lookup = self
            .entity_index
            .remove(&entity_id)
            .ok_or(ComponentErrors::EntityNotFound(entity_id))?;

        for (type_id, _) in type_lookup {
            let lookup_id = self
                .component_index
                .get_mut(&type_id)
                .ok_or(ComponentErrors::ComponentNotFound(type_id))?
                .remove(&entity_id)
                .ok_or(ComponentErrors::EntityNotFound(entity_id))?;
            self.components
                .remove(&lookup_id)
                .ok_or(ComponentErrors::ComponentLookupNotFound(lookup_id))?;
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

    #[test]
    fn adding_and_removing_component_works_correctly() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: f32,
            y: f32,
        }

        let mut cm = ChaosComponentManager::default();
        let entity_id = cm.create_entity();

        // Add component
        assert!(
            cm.add_component(entity_id, Position { x: 1.0, y: 2.0 })
                .is_ok()
        );
        let component = cm.get_component::<Position>(entity_id).unwrap();
        assert_eq!(component.x, 1.0);
        assert_eq!(component.y, 2.0);

        // Remove component
        assert!(cm.remove_component::<Position>(entity_id).is_ok());
        assert!(cm.get_component::<Position>(entity_id).is_err());
    }

    #[test]
    fn removing_entity_removes_all_its_components() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: f32,
            y: f32,
        }

        #[derive(Clone, PartialEq, Debug)]
        struct Velocity {
            dx: f32,
            dy: f32,
        }

        let mut cm = ChaosComponentManager::default();
        let entity_id = cm.create_entity();

        // Add components
        assert!(
            cm.add_component(entity_id, Position { x: 1.0, y: 2.0 })
                .is_ok()
        );
        assert!(
            cm.add_component(entity_id, Velocity { dx: 0.5, dy: 0.5 })
                .is_ok()
        );

        // Remove entity
        assert!(cm.remove_entity(entity_id).is_ok());

        // Ensure components are removed
        assert!(cm.get_component::<Position>(entity_id).is_err());
        assert!(cm.get_component::<Velocity>(entity_id).is_err());
    }

    #[test]
    fn get_all_components_returns_correct_values() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: f32,
            y: f32,
        }

        let mut cm = ChaosComponentManager::default();
        let entity_id1 = cm.create_entity();
        let entity_id2 = cm.create_entity();

        // Add components
        assert!(
            cm.add_component(entity_id1, Position { x: 1.0, y: 2.0 })
                .is_ok()
        );
        assert!(
            cm.add_component(entity_id2, Position { x: 3.0, y: 4.0 })
                .is_ok()
        );

        // Get all components
        let components = cm.get_all_components::<Position>().unwrap();
        assert_eq!(components.len(), 2);
        assert!(components.contains(&(&entity_id1, &Position { x: 1.0, y: 2.0 })));
        assert!(components.contains(&(&entity_id2, &Position { x: 3.0, y: 4.0 })));
    }

    #[test]
    fn get_component_mut_allows_modification() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: f32,
            y: f32,
        }

        let mut cm = ChaosComponentManager::default();
        let entity_id = cm.create_entity();

        // Add component
        assert!(
            cm.add_component(entity_id, Position { x: 1.0, y: 2.0 })
                .is_ok()
        );

        // Modify component
        let component = cm.get_component_mut::<Position>(entity_id).unwrap();
        component.x = 10.0;
        component.y = 20.0;

        // Verify modification
        let updated_component = cm.get_component::<Position>(entity_id).unwrap();
        assert_eq!(updated_component.x, 10.0);
        assert_eq!(updated_component.y, 20.0);
    }

    #[test]
    fn adding_duplicate_component_overwrites_existing() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: f32,
            y: f32,
        }

        let mut cm = ChaosComponentManager::default();
        let entity_id = cm.create_entity();

        // Add initial component
        assert!(
            cm.add_component(entity_id, Position { x: 1.0, y: 2.0 })
                .is_ok()
        );

        // Add duplicate component
        assert!(
            cm.add_component(entity_id, Position { x: 3.0, y: 4.0 })
                .is_ok()
        );

        // Verify the component was overwritten
        let component = cm.get_component::<Position>(entity_id).unwrap();
        assert_eq!(component.x, 3.0);
        assert_eq!(component.y, 4.0);
        assert_eq!(cm.components.len(), 1);
    }

    #[test]
    fn removing_missing_component_returns_err() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: f32,
            y: f32,
        }

        let mut cm = ChaosComponentManager::default();
        let entity_id = cm.create_entity();

        assert_eq!(
            cm.remove_component::<Position>(entity_id),
            Err(ComponentErrors::ComponentNotFound(TypeId::of::<Position>()))
        );
    }
}
