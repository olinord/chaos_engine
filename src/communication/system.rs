use crate::ecs::system::ChaosSystem;
use chaos_communicator::communicator::{ChaosCommunicator, ChaosCommunicatorError};
use chaos_communicator::message::ChaosMessage;

struct ChaosCommunicationSystem {
    communicator: ChaosCommunicator,
}

impl ChaosCommunicationSystem {
    pub fn new(communicator: ChaosCommunicator) -> ChaosCommunicationSystem {
        ChaosCommunicationSystem { communicator }
    }

    pub fn send_message(&mut self, message: ChaosMessage) -> Result<(), ChaosCommunicatorError> {
        self.communicator.send_message(message)
    }

    pub fn register_for<T: Hash>(&mut self, event: T) -> ChaosReceiver {
        self.communicator.register_for(event)
    }
}

impl ChaosSystem for ChaosCommunicationSystem {
    fn initialize(&mut self, _component_manager: &mut crate::ecs::manager::ChaosComponentManager) {}

    fn update(
        &mut self,
        _delta_time: f32,
        _component_manager: &mut crate::ecs::manager::ChaosComponentManager,
    ) -> Result<(), &'static str> {
        // Update logic if needed
        Ok(())
    }
}
