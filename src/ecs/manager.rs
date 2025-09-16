use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;

extern crate chaos_communicator;

use crate::ecs::component::Component;
use crate::ecs::errors::ComponentErrors;
use crate::ecs::errors::ComponentErrors::{
    ComponentCastError, ComponentLookupNotFound, ComponentNotFound, EntityNotFound,
};
use crate::ecs::system::ChaosSystem;
use crate::ecs::{EntityID, LookupID};
use chaos_communicator::communicator::ChaosCommunicator;
use chaos_communicator::communicator::ChaosReceiver;
use chaos_communicator::message::ChaosMessageBuilder;

#[derive(Debug)]
pub enum ChaosComponentManagerErrors {
    SystemDependencyError(String),
}

pub struct ChaosComponentManager {
    entity_index: HashMap<EntityID, HashMap<TypeId, LookupID>>,
    component_index: HashMap<TypeId, HashMap<EntityID, LookupID>>,
    components: HashMap<LookupID, Box<dyn Any>>,
    current_entity_id: EntityID,
    current_index_id: LookupID,
    communicator: ChaosCommunicator,
    systems: HashMap<TypeId, Box<dyn Any>>,
    dependency_graph: Vec<(TypeId, Vec<TypeId>)>,
}

impl ChaosComponentManager {
    pub fn default() -> ChaosComponentManager {
        return ChaosComponentManager::new(5, 5);
    }

    pub fn new(
        initial_entity_capacity: usize,
        initial_component_capacity: usize,
    ) -> ChaosComponentManager {
        ChaosComponentManager {
            entity_index: HashMap::with_capacity(initial_entity_capacity),
            component_index: HashMap::with_capacity(initial_component_capacity),
            components: HashMap::with_capacity(initial_component_capacity),
            current_entity_id: 0,
            current_index_id: 0,
            communicator: ChaosCommunicator::new(),
            systems: HashMap::new(),
            dependency_graph: Vec::new(),
        }
    }

    pub fn subscribe_to_add<T: Component>(&mut self) -> ChaosReceiver {
        self.communicator
            .register_for(format!("add_{}", type_name::<T>()))
    }

    pub fn subscribe_to_remove<T: Component>(&mut self) -> ChaosReceiver {
        self.communicator
            .register_for(format!("remove_{}", type_name::<T>()))
    }

    pub fn add_system<T: ChaosSystem>(&mut self, system: T) {
        log::info!("Adding system: {}", type_name::<T>());
        let dependencies = system.get_dependencies();
        self.systems.insert(TypeId::of::<T>(), Box::new(system));
        self.dependency_graph
            .push((TypeId::of::<T>(), dependencies));
    }

    pub fn update_system_order(&mut self) -> Result<(), ChaosComponentManagerErrors> {
        // algorithm:
        // 1. add all systems with no dependencies to the stack
        // 2. for each system with dependencies, check if all dependencies are in the stack
        // 3. if all dependencies are in the stack, add the system to the stack
        // 4. if not, add the system to the end of the system list
        let mut stack: Vec<TypeId> = Vec::new();

        let mut dependency_clone = self.dependency_graph.clone();

        let mut circular_dependency_check: Vec<TypeId> = Vec::new();

        while let Some((type_id, dependencies)) = dependency_clone.pop() {
            if dependencies.is_empty() {
                // no dependencies, add to stack
                stack.push(type_id);
            } else {
                // check if the dependencies are already in the stack
                let mut all_on_stack = true;
                for dep in &dependencies {
                    if !stack.contains(dep) {
                        all_on_stack = false;
                        break;
                    }
                }
                if !all_on_stack {
                    if circular_dependency_check.contains(&type_id) {
                        return Err(ChaosComponentManagerErrors::SystemDependencyError(format!(
                            "Circular dependency detected for system: {:?}\nDependency tree: {:?}",
                            type_id, dependency_clone
                        )));
                    }
                    // not all dependencies are on the stack, reinsert at end of list
                    dependency_clone.insert(0, (type_id, dependencies));
                    circular_dependency_check.push(type_id);
                } else {
                    stack.push(type_id);
                }
            }
        }

        let dependency_clone = stack
            .iter()
            .map(|id| {
                self.dependency_graph
                    .iter()
                    .find(|(tid, _)| tid == id)
                    .cloned()
                    .unwrap_or((*id, Vec::new()))
            })
            .collect::<Vec<_>>();

        self.dependency_graph = dependency_clone;

        Ok(())
    }

    pub fn get_system<T: ChaosSystem>(&self) -> Option<&T> {
        let dyn_system = self.systems.get(&TypeId::of::<T>())?;
        dyn_system.downcast_ref::<T>()
    }

    pub fn get_system_mut<T: ChaosSystem>(&mut self) -> Option<&mut T> {
        let dyn_system = self.systems.get_mut(&TypeId::of::<T>())?;
        dyn_system.downcast_mut::<T>()
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
    pub fn add_component<T: Component>(
        &mut self,
        entity_id: u64,
        component: T,
    ) -> Result<(), ComponentErrors> {
        match self.entity_index.get_mut(&entity_id) {
            Some(e) => {
                let id = self.current_index_id;
                self.current_index_id += 1;

                e.insert(component.type_id(), id);

                self.component_index
                    .entry(TypeId::of::<T>())
                    .or_insert(HashMap::new())
                    .insert(entity_id, id);
                self.components.insert(id, Box::new(component));

                let _ = self.communicator.send_message(
                    ChaosMessageBuilder::new()
                        .with_param("entity_id", entity_id)
                        .build_for_event(format!("add_{}", type_name::<T>())),
                );
                Ok(())
            }
            None => Err(EntityNotFound(entity_id)),
        }
    }

    fn lookup_component<T: Component>(&self, lookup_id: &LookupID) -> Result<&T, ComponentErrors> {
        match self.components.get(&lookup_id) {
            Some(component) => Ok(component
                .downcast_ref::<T>()
                .ok_or(ComponentCastError(TypeId::of::<T>()))?),
            None => Err(ComponentLookupNotFound(lookup_id.clone())),
        }
    }

    fn lookup_component_mut<T: Component>(
        &mut self,
        lookup_id: &LookupID,
    ) -> Result<&mut T, ComponentErrors> {
        match self.components.get_mut(&lookup_id) {
            Some(component) => Ok(component
                .downcast_mut::<T>()
                .ok_or(ComponentCastError(TypeId::of::<T>()))?),
            None => Err(ComponentLookupNotFound(lookup_id.clone())),
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
            None => Err(ComponentNotFound(TypeId::of::<T>())),
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
            None => Err(ComponentNotFound(TypeId::of::<T>())),
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
    pub fn get_component_mut<T: Component>(
        &mut self,
        entity_id: u64,
    ) -> Result<&mut T, ComponentErrors> {
        return self.lookup_component_mut::<T>(&self.get_lookup_id::<T>(entity_id)?.clone());
    }

    /// Removes a component from an entity
    pub fn remove_component<T: Component>(
        &mut self,
        entity_id: EntityID,
    ) -> Result<(), ComponentErrors> {
        let type_id = TypeId::of::<T>();
        let lookup_id = self
            .entity_index
            .get_mut(&entity_id)
            .ok_or(EntityNotFound(entity_id))
            .unwrap()
            .get_mut(&type_id)
            .ok_or(ComponentNotFound(type_id))
            .unwrap();

        // remove from the component lookup table
        self.components.remove(lookup_id);

        let component_type = TypeId::of::<T>();

        // remove from the component list for the entity
        let entity_component_lookup = self
            .entity_index
            .get_mut(&entity_id)
            .ok_or(EntityNotFound(entity_id))?;
        entity_component_lookup.remove(&component_type);

        // remove from the entity list for the component type
        let component_entity_lookup = self
            .component_index
            .get_mut(&component_type)
            .ok_or(ComponentNotFound(component_type))?;
        component_entity_lookup.remove(&entity_id);

        let _ = self.communicator.send_message(
            ChaosMessageBuilder::new()
                .with_param("entity_id", entity_id)
                .build_for_event(format!("remove_{}", type_name::<T>())),
        );
        return Ok(());
    }

    /// Removes an entity
    pub fn remove_entity(&mut self, entity_id: EntityID) -> Result<(), ComponentErrors> {
        let type_lookup = self
            .entity_index
            .remove(&entity_id)
            .ok_or(EntityNotFound(entity_id))?;

        for (type_id, _) in type_lookup {
            let lookup_id = self
                .component_index
                .get_mut(&type_id)
                .ok_or(ComponentNotFound(type_id))?
                .remove(&entity_id)
                .ok_or(EntityNotFound(entity_id))?;
            self.components
                .remove(&lookup_id)
                .ok_or(ComponentLookupNotFound(lookup_id))?;
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
    fn system_are_sorted_in_dependency_order() -> Result<(), ChaosComponentManagerErrors> {
        struct A;
        impl ChaosSystem for A {
            fn initialize(&mut self, _component_manager: &mut ChaosComponentManager) {}
            fn update(
                &mut self,
                _delta_time: f32,
                _component_manager: &mut ChaosComponentManager,
            ) -> Result<(), &'static str> {
                Ok(())
            }
        }

        struct B;
        impl ChaosSystem for B {
            fn initialize(&mut self, _component_manager: &mut ChaosComponentManager) {}
            fn update(
                &mut self,
                _delta_time: f32,
                _component_manager: &mut ChaosComponentManager,
            ) -> Result<(), &'static str> {
                Ok(())
            }

            fn get_dependencies(&self) -> Vec<TypeId> {
                vec![TypeId::of::<A>()]
            }
        }

        struct C;
        impl ChaosSystem for C {
            fn initialize(&mut self, _component_manager: &mut ChaosComponentManager) {}
            fn update(
                &mut self,
                _delta_time: f32,
                _component_manager: &mut ChaosComponentManager,
            ) -> Result<(), &'static str> {
                Ok(())
            }

            fn get_dependencies(&self) -> Vec<TypeId> {
                vec![TypeId::of::<B>()]
            }
        }

        let mut cm = ChaosComponentManager::default();
        cm.add_system(C {});
        cm.add_system(A {});
        cm.add_system(B {});

        cm.update_system_order()?;
        assert_eq!(3, cm.dependency_graph.len());
        assert_eq!(TypeId::of::<A>(), cm.dependency_graph[0].0);
        assert_eq!(TypeId::of::<B>(), cm.dependency_graph[1].0);
        assert_eq!(TypeId::of::<C>(), cm.dependency_graph[2].0);
        Ok(())
    }

    #[test]
    fn systems_with_cirular_dependency_return_err() {
        struct A;
        impl ChaosSystem for A {
            fn initialize(&mut self, _component_manager: &mut ChaosComponentManager) {}
            fn update(
                &mut self,
                _delta_time: f32,
                _component_manager: &mut ChaosComponentManager,
            ) -> Result<(), &'static str> {
                Ok(())
            }

            fn get_dependencies(&self) -> Vec<TypeId> {
                vec![TypeId::of::<C>()]
            }
        }

        struct B;
        impl ChaosSystem for B {
            fn initialize(&mut self, _component_manager: &mut ChaosComponentManager) {}
            fn update(
                &mut self,
                _delta_time: f32,
                _component_manager: &mut ChaosComponentManager,
            ) -> Result<(), &'static str> {
                Ok(())
            }

            fn get_dependencies(&self) -> Vec<TypeId> {
                vec![TypeId::of::<A>()]
            }
        }

        struct C;
        impl ChaosSystem for C {
            fn initialize(&mut self, _component_manager: &mut ChaosComponentManager) {}
            fn update(
                &mut self,
                _delta_time: f32,
                _component_manager: &mut ChaosComponentManager,
            ) -> Result<(), &'static str> {
                Ok(())
            }

            fn get_dependencies(&self) -> Vec<TypeId> {
                vec![TypeId::of::<B>()]
            }
        }

        let mut cm = ChaosComponentManager::default();
        cm.add_system(C {});
        cm.add_system(A {});
        cm.add_system(B {});

        assert!(cm.update_system_order().is_err());
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
    }

    #[test]
    fn system_retreival_returns_correct_instance() {
        struct TestSystem;
        impl ChaosSystem for TestSystem {
            fn initialize(&mut self, _component_manager: &mut ChaosComponentManager) {}
            fn update(
                &mut self,
                _delta_time: f32,
                _component_manager: &mut ChaosComponentManager,
            ) -> Result<(), &'static str> {
                Ok(())
            }
        }

        let mut cm = ChaosComponentManager::default();
        cm.add_system(TestSystem {});

        let system = cm.get_system::<TestSystem>();
        assert!(system.is_some());
        assert_eq!(TypeId::of::<TestSystem>(), system.unwrap().type_id());

        assert!(cm.get_system_mut::<TestSystem>().is_some());
    }
}
