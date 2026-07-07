use std::{
    any::{Any, TypeId, type_name},
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use chaos_communicator::{
    communicator::{ChaosCommunicator, ChaosReceiver},
    message::ChaosMessageBuilder,
};

use crate::ecs::{
    EntityID,
    componentstore::{ComponentStore, ErasedComponentStore},
    errors::ComponentErrors,
    query::{
        QueryAccess, QueryError, QueryIter, QueryStoreBorrow, QueryTuple, validate_query_accesses,
    },
};

pub trait Component: Any {}
impl<T: Any> Component for T {}

pub struct ChaosComponentManager {
    entities: HashSet<EntityID>,
    component_stores: HashMap<TypeId, Box<dyn ErasedComponentStore>>,
    current_entity_id: EntityID,
    communicator: Arc<Mutex<ChaosCommunicator>>,
}

impl Default for ChaosComponentManager {
    fn default() -> Self {
        ChaosComponentManager::new(Arc::new(Mutex::new(ChaosCommunicator::new())))
    }
}

impl ChaosComponentManager {
    pub fn new(communicator: Arc<Mutex<ChaosCommunicator>>) -> ChaosComponentManager {
        ChaosComponentManager {
            entities: HashSet::new(),
            component_stores: HashMap::new(),
            current_entity_id: 0,
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

    fn store<T: Component>(&self) -> Option<&ComponentStore<T>> {
        self.component_stores
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref::<ComponentStore<T>>()
    }

    fn store_mut<T: Component>(&mut self) -> Option<&mut ComponentStore<T>> {
        self.component_stores
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut::<ComponentStore<T>>()
    }

    fn store_mut_or_insert<T: Component>(&mut self) -> &mut ComponentStore<T> {
        self.component_stores
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(ComponentStore::<T>::new()))
            .as_any_mut()
            .downcast_mut::<ComponentStore<T>>()
            .unwrap()
    }

    fn borrow_query_stores<'world>(
        &'world mut self,
        accesses: &[QueryAccess],
    ) -> QueryStoreBorrow<'world> {
        let mut stores = QueryStoreBorrow::new();

        for access in accesses {
            match access.kind {
                crate::ecs::query::QueryAccessKind::Read => {
                    if let Some(store) = self.component_stores.get(&access.type_id) {
                        stores.insert_read(access.type_id, store.as_ref());
                    }
                }
                crate::ecs::query::QueryAccessKind::Write => {
                    if let Some(store) = self.component_stores.get_mut(&access.type_id) {
                        stores.insert_write(access.type_id, store.as_mut());
                    }
                }
            }
        }

        stores
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

        self.entities.insert(id);

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
        if self.entities.get(&entity_id).is_none() {
            return Err(ComponentErrors::EntityNotFound(entity_id));
        }

        // insert will replace the existing component if it already exists for the entity
        self.store_mut_or_insert::<T>().insert(entity_id, component);

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

        return Ok(());
    }

    /// Removes a component from an entity
    pub fn remove_component<T: Component>(
        &mut self,
        entity_id: EntityID,
    ) -> Result<(), ComponentErrors> {
        self.store_mut::<T>()
            .ok_or(ComponentErrors::ComponentNorRegistered(
                stringify!(T).into(),
            ))?
            .remove_entity(entity_id);

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
    pub fn remove_entity(&mut self, entity_id: EntityID) {
        for store in self.component_stores.values_mut() {
            store.remove_entity(entity_id);
        }
        self.entities.remove(&entity_id);
    }

    pub fn get_component<T: Component>(&self, entity_id: EntityID) -> Option<&T> {
        match self.store::<T>() {
            Some(store) => store.get(entity_id),
            None => None,
        }
    }

    pub fn get_component_mut<T: Component>(&mut self, entity_id: EntityID) -> Option<&mut T> {
        match self.store_mut::<T>() {
            Some(store) => store.get_mut(entity_id),
            None => None,
        }
    }

    pub fn get_all_components_of_type<T: Component>(
        &self,
    ) -> Result<Vec<(EntityID, &T)>, ComponentErrors> {
        match self.store::<T>() {
            Some(store) => Ok(store.entity_values().collect()),
            None => Err(ComponentErrors::ComponentNotFound(TypeId::of::<T>())),
        }
    }

    pub fn get_all_mut_components_of_type<T: Component>(
        &mut self,
    ) -> Result<Vec<(EntityID, &mut T)>, ComponentErrors> {
        match self.store_mut::<T>() {
            Some(store) => Ok(store.entity_values_mut().collect()),
            None => Err(ComponentErrors::ComponentNotFound(TypeId::of::<T>())),
        }
    }

    pub fn entities_matching(&self, accesses: &[QueryAccess]) -> Vec<EntityID> {
        let mut type_ids = Vec::new();
        for access in accesses {
            if !type_ids.contains(&access.type_id) {
                type_ids.push(access.type_id);
            }
        }

        if type_ids.is_empty() {
            return Vec::new();
        }

        let mut driver_entities: Option<Vec<EntityID>> = None;
        for type_id in &type_ids {
            let Some(store) = self.component_stores.get(type_id) else {
                return Vec::new();
            };

            let entities = store.entities();
            if driver_entities
                .as_ref()
                .is_none_or(|driver| entities.len() < driver.len())
            {
                driver_entities = Some(entities);
            }
        }

        driver_entities
            .unwrap_or_default()
            .into_iter()
            .filter(|entity| {
                type_ids.iter().all(|type_id| {
                    self.component_stores
                        .get(type_id)
                        .is_some_and(|store| store.has_entity(*entity))
                })
            })
            .collect()
    }

    pub fn query<'world, Q>(&'world mut self) -> Result<QueryIter<'world, Q>, QueryError>
    where
        Q: QueryTuple<'world>,
    {
        let accesses = Q::accesses();
        validate_query_accesses(&accesses)?;
        let entity_ids = self.entities_matching(&accesses);
        let stores = self.borrow_query_stores(&accesses);

        Ok(QueryIter::new(entity_ids, stores))
    }

    pub fn query_for_entity<'world, Q>(&'world mut self, entity_id: EntityID) -> Option<Q::Item>
    where
        Q: QueryTuple<'world>,
    {
        let accesses = Q::accesses();
        if validate_query_accesses(&accesses).is_err() {
            return None;
        }
        let stores = self.borrow_query_stores(&accesses);

        Q::fetch(&stores, entity_id)
    }

    pub fn for_each<'world, Q, F>(&'world mut self, mut f: F) -> Result<(), QueryError>
    where
        Q: QueryTuple<'world>,
        F: FnMut(EntityID, Q::Item),
    {
        for (entity, item) in self.query::<Q>()? {
            f(entity, item);
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
        assert!(cm.get_component::<Person>(entity_id).is_none());
        assert!(cm.get_component_mut::<Person>(entity_id).is_none());
    }

    #[test]
    fn looking_up_component_that_doesnt_exist_returns_err() {
        let mut cm = ChaosComponentManager::default();
        let entity_id: EntityID = cm.create_entity();
        assert!(cm.get_component::<Person>(entity_id).is_none());
        assert!(cm.get_component_mut::<Person>(entity_id).is_none());
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
        assert!(cm.get_component::<Position>(entity_id).is_none());
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
        cm.remove_entity(entity_id);

        // Ensure components are removed
        assert!(cm.get_component::<Position>(entity_id).is_none());
        assert!(cm.get_component::<Velocity>(entity_id).is_none());
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
        let components = cm.store::<Position>().unwrap();
        assert_eq!(components.len(), 2);
        assert_eq!(
            components.get(entity_id1),
            Some(&Position { x: 1.0, y: 2.0 })
        );
        assert_eq!(
            components.get(entity_id2),
            Some(&Position { x: 3.0, y: 4.0 })
        );
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
        let store = cm.store::<Position>().unwrap();
        assert_eq!(store.len(), 1); // Ensure only one component exists for the entity
    }

    #[test]
    fn immutable_query_returns_entities_with_all_requested_components() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: i32,
        }

        #[derive(Clone, PartialEq, Debug)]
        struct Velocity {
            dx: i32,
        }

        let mut cm = ChaosComponentManager::default();
        let moving_entity = cm.create_entity();
        let stationary_entity = cm.create_entity();

        cm.add_component(moving_entity, Position { x: 1 }).unwrap();
        cm.add_component(moving_entity, Velocity { dx: 2 }).unwrap();
        cm.add_component(stationary_entity, Position { x: 10 })
            .unwrap();

        let values: Vec<_> = cm
            .query::<(&Position, &Velocity)>()
            .unwrap()
            .map(|(_entity, (position, velocity))| (position.x, velocity.dx))
            .collect();

        assert_eq!(values, vec![(1, 2)]);
    }

    #[test]
    fn mutable_query_updates_matching_components() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: i32,
        }

        #[derive(Clone, PartialEq, Debug)]
        struct Velocity {
            dx: i32,
        }

        let mut cm = ChaosComponentManager::default();
        let moving_entity = cm.create_entity();
        let stationary_entity = cm.create_entity();

        cm.add_component(moving_entity, Position { x: 1 }).unwrap();
        cm.add_component(moving_entity, Velocity { dx: 2 }).unwrap();
        cm.add_component(stationary_entity, Position { x: 10 })
            .unwrap();

        {
            for (_entity, (position, velocity)) in cm.query::<(&mut Position, &Velocity)>().unwrap()
            {
                position.x += velocity.dx;
            }
        }

        assert_eq!(cm.get_component::<Position>(moving_entity).unwrap().x, 3);
        assert_eq!(
            cm.get_component::<Position>(stationary_entity).unwrap().x,
            10
        );
    }

    #[test]
    fn query_without_matching_store_returns_empty_results() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: i32,
        }

        #[derive(Clone, PartialEq, Debug)]
        struct Velocity {
            dx: i32,
        }

        let mut cm = ChaosComponentManager::default();
        let entity = cm.create_entity();
        cm.add_component(entity, Position { x: 1 }).unwrap();

        assert_eq!(cm.query::<(&Position, &Velocity)>().unwrap().count(), 0);
    }

    #[test]
    fn query_rejects_conflicting_access_to_the_same_component_type() {
        #[derive(Clone, PartialEq, Debug)]
        struct Position {
            x: i32,
        }

        let mut cm = ChaosComponentManager::default();

        assert!(matches!(
            cm.query::<(&mut Position, &Position)>(),
            Err(crate::ecs::query::QueryError::ConflictingAccess(_))
        ));
    }
}
