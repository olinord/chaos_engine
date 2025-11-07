use std::{
    any::{TypeId, type_name},
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chaos_communicator::communicator::ChaosCommunicator;

use crate::ecs::{component::ChaosComponentManager, system::ChaosSystem};

pub struct ChaosWorld {
    component_manager: ChaosComponentManager,
    systems: HashMap<TypeId, Box<dyn ChaosSystem>>,
    communicator: Arc<Mutex<ChaosCommunicator>>,
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

    pub fn send_message(&mut self, message: chaos_communicator::message::ChaosMessage) {
        let mut guard = self.communicator.lock();
        match guard {
            Ok(ref mut comm) => {
                comm.send_message(message).expect("Failed to send message");
            }
            Err(_) => panic!("Failed to acquire communicator lock"),
        };
    }

    pub fn register_for<T: std::hash::Hash>(
        &mut self,
        event: T,
    ) -> chaos_communicator::communicator::ChaosReceiver {
        let mut guard = self.communicator.lock();
        match guard {
            Ok(ref mut comm) => return comm.register_for(event),
            Err(_) => panic!("Failed to acquire communicator lock"),
        };
    }

    pub fn add_system<T: ChaosSystem>(&mut self, system: T) {
        log::info!("Adding system: {}", type_name::<T>());
        self.systems.insert(TypeId::of::<T>(), Box::new(system));
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), &'static str> {
        for (_, system) in self.systems.iter_mut() {
            system.update(delta_time, &mut self.component_manager)?;
        }
        Ok(())
    }
}
