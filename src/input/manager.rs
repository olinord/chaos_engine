use std::any::{Any, TypeId};
use std::collections::HashMap;
use commands::cmd::ExitCmd;
use input::events::{ChaosDeviceDetailedEvent, ChaosDeviceEventState, KeyCode, ChaosDeviceEvent};
use input::events::ChaosDeviceEventRegistration;

pub struct ChaosDeviceEventManager {
    registered_commands: HashMap<ChaosDeviceEventRegistration, TypeId>,
    registered_events: HashMap<ChaosDeviceEvent, (ChaosDeviceEventRegistration, Option<ChaosDeviceEventState>)>,
}

const MULTI_PRESS_TIME_IN_MS : u128= 300;

impl ChaosDeviceEventManager {
    pub fn new() -> ChaosDeviceEventManager {
        let mut cdem = ChaosDeviceEventManager {
            registered_commands: HashMap::new(),
            registered_events: HashMap::new(),
        };

        // Default event registrations.
        // Need to know when to close the damn thing
        cdem.register_command::<ExitCmd>(ChaosDeviceEventRegistration::CloseRequested);

        return cdem;
    }

    pub fn register_single_key_press<T: Any>(&mut self, input_key: KeyCode) -> bool {
        let event = ChaosDeviceEvent::KeyPress(input_key);

        if self.is_registered_event(&event) {
            return false;
        }

        self.registered_commands.insert(ChaosDeviceEventRegistration::KeyPress(input_key), TypeId::of::<T>());
        self.registered_events.insert(event,
                                      (
                                          ChaosDeviceEventRegistration::KeyPress(input_key),
                                          Some(ChaosDeviceEventState::new_single(input_key))
                                      ),
        );
        return true;
    }

    pub fn register_multi_key_press<T: Any>(&mut self, input_key: KeyCode, repeats: u128) -> bool {
        let event = ChaosDeviceEvent::KeyPress(input_key);
        if self.is_registered_event(&event) {
            return false;
        }

        self.registered_commands.insert(ChaosDeviceEventRegistration::MultiKeyPress(input_key, repeats), TypeId::of::<T>());
        self.registered_events.insert(event,
                                      (
                                          ChaosDeviceEventRegistration::MultiKeyPress(input_key, repeats),
                                          Some(ChaosDeviceEventState::new_multi(input_key, repeats))
                                      ),
        );
        return true;
    }

    pub fn register_command<T: Any>(&mut self, event_registration: ChaosDeviceEventRegistration) -> bool {
        let event = ChaosDeviceEvent::from(ChaosDeviceDetailedEvent::from(event_registration));

        if self.is_registered_event(&event) {
            return false;
        }

        self.registered_commands.insert(event_registration, TypeId::of::<T>());
        self.registered_events.insert(event, (event_registration.clone(), None));

        return true;
    }

    pub fn get_commands(&mut self, events: &Vec<ChaosDeviceDetailedEvent>) -> Vec<TypeId> {
        let mut commands = Vec::new();
        for chaos_device_event in events {
            // get a lookup key that doesn't contain details about key presses or new window size
            let event_lookup_key = ChaosDeviceEvent::from(chaos_device_event);
            match self.registered_events.get_mut(&event_lookup_key) {
                Some((registration, state)) => {
                    match state {
                        Some(button_state) => {
                            // If this is a button specific event, handle that here
                            button_state.update(&chaos_device_event, MULTI_PRESS_TIME_IN_MS);

                            // We can do this because of the partial eq impl for ChaosDeviceEventRegistration
                            if button_state == registration {
                                if let Some(command) = self.registered_commands.get(registration) {
                                    commands.push(command.clone());
                                }
                            }
                        }
                        None => {
                            // Maybe this was a non button related event
                            // like close window, resize etc
                            if let Some(command) = self.registered_commands.get(registration) {
                                commands.push(command.clone());
                            }
                        }
                    }
                }
                None => ()
            };
        }
        return commands;
    }

    fn is_registered_event(&self, chaos_device_event: &ChaosDeviceEvent) -> bool {
        return self.registered_events.contains_key(chaos_device_event);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    struct TestCmd {}


    #[test]
    fn initialized_command_manager_has_exit_cmd_registered_for_close_requested() {
        let event_manager = ChaosDeviceEventManager::new();
        assert!(event_manager.is_registered_event(&ChaosDeviceEvent::CloseRequested));
    }

    #[test]
    fn single_button_click_can_be_registered() {
        let mut event_manager = ChaosDeviceEventManager::new();
        assert!(!event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(KeyCode::Escape)));

        event_manager.register_single_key_press::<TestCmd>(KeyCode::Escape);
        assert!(event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(KeyCode::Escape)));
    }

    #[test]
    fn multi_button_click_can_be_registered() {
        let mut event_manager = ChaosDeviceEventManager::new();
        assert!(!event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(KeyCode::Escape)));

        event_manager.register_multi_key_press::<TestCmd>(KeyCode::Escape, 2);
        assert!(event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(KeyCode::Escape)));
    }

    #[test]
    fn any_event_can_be_manually_registered() {
        let mut event_manager = ChaosDeviceEventManager::new();
        assert!(!event_manager.is_registered_event(&ChaosDeviceEvent::Focused));

        event_manager.register_command::<TestCmd>(ChaosDeviceEventRegistration::Focused );
        assert!(event_manager.is_registered_event(&ChaosDeviceEvent::Focused));
    }

    #[test]
    fn cmd_is_returned_from_get_commands_when_event_matches_single_key_press() {
        let mut event_manager = ChaosDeviceEventManager::new();
        event_manager.register_single_key_press::<TestCmd>(KeyCode::Escape);
        let commands = event_manager.get_commands(&vec![ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true)]);
        assert!( commands.contains(&TypeId::of::<TestCmd>()));
    }

    #[test]
    fn cmd_is_returned_from_get_commands_when_event_matches_multi_key_press() {
        let mut event_manager = ChaosDeviceEventManager::new();
        event_manager.register_multi_key_press::<TestCmd>(KeyCode::Escape, 2);

        let commands = event_manager.get_commands(&vec![ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true)]);
        assert!( !commands.contains(&TypeId::of::<TestCmd>()));
        let commands = event_manager.get_commands(&vec![ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, false)]);
        assert!( !commands.contains(&TypeId::of::<TestCmd>()));
        let commands = event_manager.get_commands(&vec![ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true)]);
        assert!( commands.contains(&TypeId::of::<TestCmd>()));
    }
}

