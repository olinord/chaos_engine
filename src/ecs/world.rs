use std::{
    any::{TypeId, type_name},
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chaos_communicator::{
    communicator::{ChaosCommunicationError, ChaosCommunicator, ChaosReceiver},
    message::ChaosMessage,
};

use crate::ecs::{
    EntityID,
    component::{ChaosComponentManager, Component},
    entity::EntityBuilder,
    errors::ComponentErrors,
    system::ChaosSystem,
};

pub struct ChaosWorld {
    component_manager: ChaosComponentManager,
    systems: HashMap<TypeId, Box<dyn ChaosSystem>>,
    communicator: Arc<Mutex<ChaosCommunicator>>,
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
            component_manager: ChaosComponentManager::new(100, 10, communicator.clone()),
            systems: HashMap::new(),
            communicator,
        }
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

    pub fn add_system<T: ChaosSystem>(&mut self, system: T) {
        log::info!("Adding system: {}", type_name::<T>());
        self.systems.insert(TypeId::of::<T>(), Box::new(system));

        // get first mutable reference to the system we just added
        let system = self.systems.get_mut(&TypeId::of::<T>()).unwrap().as_mut();
        match system.initialize(&mut self.component_manager) {
            Ok(_) => (),
            Err(e) => panic!("Failed to initialize system: {}", e),
        };
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), &'static str> {
        for (_, system) in self.systems.iter_mut() {
            system.update(delta_time, &mut self.component_manager)?;
        }
        Ok(())
    }

    // creation methods
    pub fn spawn(&mut self) -> EntityBuilder<'_> {
        EntityBuilder::new(self.component_manager.create_entity(), self)
    }

    pub fn despawn(&mut self, entity: EntityID) -> Result<(), ComponentErrors> {
        self.component_manager.remove_entity(entity)
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

    pub fn get_component<T: Component>(&self, entity_id: EntityID) -> Result<&T, ComponentErrors> {
        self.component_manager.get_component::<T>(entity_id)
    }

    pub fn get_component_mut<T: Component>(
        &mut self,
        entity_id: EntityID,
    ) -> Result<&mut T, ComponentErrors> {
        self.component_manager.get_component_mut::<T>(entity_id)
    }

    pub fn get_all_components_of_type<T: Component>(&self) -> Result<Vec<&T>, ComponentErrors> {
        self.component_manager.get_all_components_of_type::<T>()
    }

    pub fn subscribe_to_add<T: Component>(&mut self) -> ChaosReceiver {
        self.component_manager.subscribe_to_add::<T>()
    }
}
