use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::HashMap;
use ecs::EntityID;
use ecs::component::Component;
use std::any::TypeId;

pub struct ChaosCommunicator {
    component_added_senders: HashMap<TypeId, Vec<Sender<EntityID>>>,
    component_removed_senders: HashMap<TypeId, Vec<Sender<EntityID>>>
}

impl ChaosCommunicator {
    pub fn new() -> ChaosCommunicator {
        ChaosCommunicator{
            component_added_senders: HashMap::new(),
            component_removed_senders: HashMap::new()
        }
    }

    pub fn register_for_add<T: Component>(&mut self) -> Receiver<EntityID> {
        let (sender, receiver) = channel();
        let type_id = TypeId::of::<T>();
        // add to list of senders
        self.component_added_senders.entry(type_id).or_insert(Vec::new()).push(sender);
        return receiver;
    }

    pub fn register_for_remove<T: Component>(&mut self) -> Receiver<EntityID> {
        let (sender, receiver) = channel();
        let type_id = TypeId::of::<T>();
        // add to list of senders
        self.component_removed_senders.entry(type_id).or_insert(Vec::new()).push(sender);
        return receiver;
    }

    pub fn notify_of_add<T: Component>(&self, entity_id: EntityID) {
        self.notify_of_type_add(&TypeId::of::<T>(), entity_id);
    }

    pub fn notify_of_type_add(&self, type_id: &TypeId, entity_id: EntityID) {
        let senders = self.component_added_senders.get(type_id);

        if senders.is_none() {
            return;
        }

        for sender in senders.unwrap() {
            let result = sender.send(entity_id);
            if let Err(error) = result {
                println!("Error sending add notification {}!", error);
            }
        }
    }

    pub fn notify_of_removal<T: Component>(&self, entity_id: EntityID) {
        self.notify_of_type_removal(&TypeId::of::<T>(), entity_id);
    }

    pub fn notify_of_type_removal(&self, type_id: &TypeId, entity_id: EntityID) {
        let senders = self.component_removed_senders.get(type_id);

        if senders.is_none() {
            return;
        }

        for sender in senders.unwrap() {
            let result = sender.send(entity_id);
            if let Err(error) = result {
                println!("Error sending remove notification {}!", error);
            }
        }
    }
}