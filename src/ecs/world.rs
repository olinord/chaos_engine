use std::{
    any::{TypeId, type_name},
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::Instant,
};

use chaos_communicator::{
    communicator::{ChaosCommunicationError, ChaosCommunicator, ChaosReceiver},
    message::ChaosMessage,
};

use crate::{
    ecs::{
        EntityID,
        component::{ChaosComponentManager, Component},
        entity::EntityBuilder,
        errors::ComponentErrors,
        query::{QueryError, QueryIter, QueryTuple},
        system::ChaosSystem,
    },
    triggers::trigger_event_key::TriggerEventKey,
};

pub struct WorldTime {
    current_time: Instant,
    last_time: Instant,
}

impl WorldTime {
    pub fn delta_time(&self) -> f32 {
        return self
            .current_time
            .duration_since(self.last_time)
            .as_secs_f32();
    }
}

pub struct ChaosWorld {
    component_manager: ChaosComponentManager,
    systems: HashMap<TypeId, Box<dyn ChaosSystem>>,
    specialized_entities: HashMap<SpecializedEntityKey, EntityID>,
    communicator: Arc<Mutex<ChaosCommunicator>>,
    time: WorldTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SpecializedEntityKey {
    key_type: TypeId,
    hash: u64,
}

impl SpecializedEntityKey {
    fn new<T: Hash + 'static>(key: &T) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        Self {
            key_type: TypeId::of::<T>(),
            hash: hasher.finish(),
        }
    }
}

impl Default for ChaosWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl ChaosWorld {
    pub fn new() -> ChaosWorld {
        let communicator = Arc::new(Mutex::new(ChaosCommunicator::new()));
        ChaosWorld {
            component_manager: ChaosComponentManager::new(communicator.clone()),
            systems: HashMap::new(),
            specialized_entities: HashMap::new(),
            communicator,
            time: WorldTime {
                current_time: Instant::now(),
                last_time: Instant::now(),
            },
        }
    }

    pub fn get_time(&self) -> &WorldTime {
        &self.time
    }

    pub fn initialize_systems(&mut self) -> Result<(), &'static str> {
        // slightly hacky way to avoid borrowing self.systems while iterating over it
        let mut systems = std::mem::take(&mut self.systems);
        for system in systems.values_mut() {
            if let Err(error) = system.initialize(self) {
                self.systems = systems;
                return Err(error);
            }
        }
        self.systems = systems;
        Ok(())
    }

    pub fn send_message(&mut self, message: ChaosMessage) {
        let mut guard = self.communicator.lock();
        match guard {
            Ok(ref mut comm) => {
                comm.send_message(message).expect("Failed to send message");
            }
            Err(_) => panic!("Failed to acquire communicator lock"),
        };
    }

    pub fn try_send_message(
        &mut self,
        message: ChaosMessage,
    ) -> Result<(), ChaosCommunicationError> {
        let guard = self
            .communicator
            .lock()
            .expect("Failed to acquire communicator lock");
        guard.send_message(message)
    }

    pub fn register_for<T: std::hash::Hash>(&mut self, event: T) -> ChaosReceiver {
        let mut guard: Result<
            std::sync::MutexGuard<'_, ChaosCommunicator>,
            std::sync::PoisonError<std::sync::MutexGuard<'_, ChaosCommunicator>>,
        > = self.communicator.lock();
        match guard {
            Ok(ref mut comm) => comm.register_for(event),
            Err(_) => panic!("Failed to acquire communicator lock"),
        }
    }

    pub fn register_for_trigger<T: std::hash::Hash>(&mut self, event: T) -> ChaosReceiver {
        self.register_for(TriggerEventKey::new(&event))
    }

    pub fn add_system<T: ChaosSystem>(&mut self, system: T) -> &mut Self {
        log::info!("Adding system: {}", type_name::<T>());

        self.systems.insert(TypeId::of::<T>(), Box::new(system));
        self
    }

    pub fn update(&mut self) -> Result<(), &'static str> {
        self.time = WorldTime {
            current_time: Instant::now(),
            last_time: self.time.current_time,
        };
        // slightly hacky way to avoid borrowing self.systems while iterating over it
        let mut systems = std::mem::take(&mut self.systems);
        for system in systems.values_mut() {
            if let Err(error) = system.update(self) {
                self.systems = systems;
                return Err(error);
            }
        }
        self.systems = systems;
        Ok(())
    }

    // creation methods
    pub fn spawn(&mut self) -> EntityBuilder<'_> {
        EntityBuilder::new(self.component_manager.create_entity(), self)
    }

    pub fn despawn(&mut self, entity: EntityID) {
        self.component_manager.remove_entity(entity);
        self.specialized_entities
            .retain(|_, registered_entity| *registered_entity != entity);
    }

    pub fn register_specialized_entity<T: Hash + 'static>(
        &mut self,
        key: T,
        entity: EntityID,
    ) -> Option<EntityID> {
        let key = SpecializedEntityKey::new(&key);
        self.specialized_entities.insert(key, entity)
    }

    pub fn get_specialized_entity<T: Hash + 'static>(&self, key: T) -> Option<EntityID> {
        let key = SpecializedEntityKey::new(&key);
        self.specialized_entities.get(&key).copied()
    }

    pub fn get_specialized_entity_component<T: Hash + 'static, C: Component>(
        &self,
        key: T,
    ) -> Option<&C> {
        let entity_id = self.get_specialized_entity(key)?;
        self.get_component::<C>(entity_id)
    }

    pub fn get_specialized_entity_component_mut<T: Hash + 'static, C: Component>(
        &mut self,
        key: T,
    ) -> Option<&mut C> {
        let entity_id = self.get_specialized_entity(key)?;
        self.get_component_mut::<C>(entity_id)
    }

    pub fn unregister_specialized_entity<T: Hash + 'static>(&mut self, key: T) -> Option<EntityID> {
        let key = SpecializedEntityKey::new(&key);
        self.specialized_entities.remove(&key)
    }

    pub fn add_component<T: Component>(
        &mut self,
        entity_id: EntityID,
        component: T,
    ) -> Result<(), ComponentErrors> {
        self.component_manager.add_component(entity_id, component)
    }

    pub fn remove_component<T: Component>(
        &mut self,
        entity_id: EntityID,
    ) -> Result<(), ComponentErrors> {
        self.component_manager.remove_component::<T>(entity_id)
    }

    pub fn get_component<T: Component>(&self, entity_id: EntityID) -> Option<&T> {
        self.component_manager.get_component::<T>(entity_id)
    }

    pub fn get_component_mut<T: Component>(&mut self, entity_id: EntityID) -> Option<&mut T> {
        self.component_manager.get_component_mut::<T>(entity_id)
    }

    pub fn get_all_components_of_type<T: Component>(
        &self,
    ) -> Result<Vec<(EntityID, &T)>, ComponentErrors> {
        self.component_manager.get_all_components_of_type::<T>()
    }

    pub fn get_all_mut_components_of_type<T: Component>(
        &mut self,
    ) -> Result<Vec<(EntityID, &mut T)>, ComponentErrors> {
        self.component_manager.get_all_mut_components_of_type::<T>()
    }

    pub fn query<'world, Q>(&'world mut self) -> Result<QueryIter<'world, Q>, QueryError>
    where
        Q: QueryTuple<'world>,
    {
        self.component_manager.query::<Q>()
    }

    pub fn query_for_entity<'world, Q>(&'world mut self, entity_id: EntityID) -> Option<Q::Item>
    where
        Q: QueryTuple<'world>,
    {
        self.component_manager.query_for_entity::<Q>(entity_id)
    }

    pub fn for_each<'world, Q, F>(&'world mut self, f: F) -> Result<(), QueryError>
    where
        Q: QueryTuple<'world>,
        F: FnMut(EntityID, Q::Item),
    {
        self.component_manager.for_each::<Q, F>(f)
    }

    pub fn subscribe_to_add<T: Component>(&mut self) -> ChaosReceiver {
        self.component_manager.subscribe_to_add::<T>()
    }

    pub fn subscribe_to_remove<T: Component>(&mut self) -> ChaosReceiver {
        self.component_manager.subscribe_to_remove::<T>()
    }
}
