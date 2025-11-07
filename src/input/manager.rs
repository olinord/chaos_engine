use winit::event::WindowEvent;

use crate::input::events::ChaosDeviceEventRegistration;
use crate::input::events::{
    ChaosDeviceDetailedEvent, ChaosDeviceEvent, ChaosDeviceEventState, ChaosKeyCode,
};
use std::any::{Any, TypeId};
use std::collections::HashMap;

#[derive(Clone)]
pub struct ChaosDeviceEventSystem {
    registered_commands: HashMap<ChaosDeviceEventRegistration, TypeId>,
    registered_events:
        HashMap<ChaosDeviceEvent, (ChaosDeviceEventRegistration, Option<ChaosDeviceEventState>)>,
    ready_registrations: Vec<ChaosDeviceEventRegistration>,
}

pub enum ChaosDeviceEventRegistrationError {
    EventAlreadyRegistered(ChaosDeviceEvent),
}

const MULTI_PRESS_TIME_IN_MS: u128 = 300;

impl ChaosDeviceEventSystem {
    pub fn new() -> ChaosDeviceEventSystem {
        ChaosDeviceEventSystem {
            registered_commands: HashMap::new(),
            registered_events: HashMap::new(),
            ready_registrations: Vec::new(),
        }
    }

    pub fn register_single_key_press<T: Any>(
        &mut self,
        input_key: ChaosKeyCode,
    ) -> Result<(), ChaosDeviceEventRegistrationError> {
        let event = ChaosDeviceEvent::KeyPress(input_key);

        if self.is_registered_event(&event) {
            return Err(ChaosDeviceEventRegistrationError::EventAlreadyRegistered(
                event,
            ));
        }

        self.registered_commands.insert(
            ChaosDeviceEventRegistration::KeyPress(input_key),
            TypeId::of::<T>(),
        );
        self.registered_events.insert(
            event,
            (
                ChaosDeviceEventRegistration::KeyPress(input_key),
                Some(ChaosDeviceEventState::new_single(input_key)),
            ),
        );
        return Ok(());
    }

    pub fn register_multi_key_press<T: Any>(
        &mut self,
        input_key: ChaosKeyCode,
        repeats: u8,
    ) -> Result<(), ChaosDeviceEventRegistrationError> {
        let event = ChaosDeviceEvent::KeyPress(input_key);
        if self.is_registered_event(&event) {
            return Err(ChaosDeviceEventRegistrationError::EventAlreadyRegistered(
                event,
            ));
        }

        self.registered_commands.insert(
            ChaosDeviceEventRegistration::MultiKeyPress(input_key, repeats),
            TypeId::of::<T>(),
        );
        self.registered_events.insert(
            event,
            (
                ChaosDeviceEventRegistration::MultiKeyPress(input_key, repeats),
                Some(ChaosDeviceEventState::new_multi(input_key, repeats)),
            ),
        );
        return Ok(());
    }

    pub fn register_command<T: Any>(
        &mut self,
        event_registration: ChaosDeviceEventRegistration,
    ) -> Result<(), ChaosDeviceEventRegistrationError> {
        let event = ChaosDeviceEvent::from(ChaosDeviceDetailedEvent::from(event_registration));

        if self.is_registered_event(&event) {
            return Err(ChaosDeviceEventRegistrationError::EventAlreadyRegistered(
                event,
            ));
        }

        self.registered_commands
            .insert(event_registration, TypeId::of::<T>());
        self.registered_events
            .insert(event, (event_registration.clone(), None));

        return Ok(());
    }

    pub fn get_ready_registrations(&self) -> Vec<ChaosDeviceEventRegistration> {
        return self.ready_registrations.clone();
    }

    pub fn clear_ready_registrations(&mut self) {
        self.ready_registrations.clear();
    }

    pub fn get_commands(&self) -> Vec<TypeId> {
        let mut commands: Vec<TypeId> = Vec::new();
        for event in self.ready_registrations.clone() {
            self.registered_commands
                .get(&event)
                .map(|cmd| commands.push(*cmd));
        }
        return commands;
    }

    pub fn update_commands(&mut self, event: &WindowEvent) {
        let chaos_event = ChaosDeviceDetailedEvent::from(event);
        let lookup = ChaosDeviceEvent::from(chaos_event.clone());
        // get a lookup key that doesn't contain details about key presses or new window size
        match self.registered_events.get_mut(&lookup) {
            Some((registration, state)) => {
                match state {
                    Some(button_state) => {
                        // If this is a button specific event, handle that here
                        button_state.update(&chaos_event, MULTI_PRESS_TIME_IN_MS);

                        // We can do this because of the partial eq impl for ChaosDeviceEventRegistration
                        if button_state == registration {
                            self.ready_registrations.push(registration.clone());
                        }
                    }
                    None => {
                        // Maybe this was a non button related event
                        // like close window, resize etc
                        self.ready_registrations.push(registration.clone());
                    }
                }
            }
            None => (),
        };
    }

    fn is_registered_event(&self, chaos_device_event: &ChaosDeviceEvent) -> bool {
        return self.registered_events.contains_key(chaos_device_event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::input::manager::WindowEvent::KeyboardInput;

    struct TestCmd {}

    #[test]
    fn initialized_command_manager_has_exit_cmd_registered_for_close_requested() {
        let event_manager = ChaosDeviceEventSystem::new();
        assert!(event_manager.is_registered_event(&ChaosDeviceEvent::CloseRequested));
    }

    #[test]
    fn single_button_click_can_be_registered() {
        let mut event_manager = ChaosDeviceEventSystem::new();
        assert!(
            !event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(ChaosKeyCode::Escape))
        );

        event_manager.register_single_key_press::<TestCmd>(ChaosKeyCode::Escape);
        assert!(
            event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(ChaosKeyCode::Escape))
        );
    }

    #[test]
    fn multi_button_click_can_be_registered() {
        let mut event_manager = ChaosDeviceEventSystem::new();
        assert!(
            !event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(ChaosKeyCode::Escape))
        );

        event_manager.register_multi_key_press::<TestCmd>(ChaosKeyCode::Escape, 2);
        assert!(
            event_manager.is_registered_event(&ChaosDeviceEvent::KeyPress(ChaosKeyCode::Escape))
        );
    }

    #[test]
    fn any_event_can_be_manually_registered() {
        let mut event_manager = ChaosDeviceEventSystem::new();
        assert!(!event_manager.is_registered_event(&ChaosDeviceEvent::Focused));

        event_manager.register_command::<TestCmd>(ChaosDeviceEventRegistration::Focused);
        assert!(event_manager.is_registered_event(&ChaosDeviceEvent::Focused));
    }
    /*
        #[test]
        fn cmd_is_returned_from_get_commands_when_event_matches_single_key_press() {
            let mut event_manager = ChaosDeviceEventSystem::new();
            event_manager.register_single_key_press::<TestCmd>(ChaosKeyCode::Escape);
            event_manager.update_commands(&WindowEvent::KeyboardInput { device_id: (), event: (), is_synthetic: () } {
                device_id: winit::event::DeviceId::dummy(),
                input: KeyboardInput {
                    scancode: 0,
                    state: winit::event::ElementState::Pressed,
                    virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
                    modifiers: winit::event::ModifiersState::empty(),
                },
                is_synthetic: false,
            });
            event_manager.get_ready_registrations();
            let commands = event_manager.get(&vec![ChaosDeviceDetailedEvent::KeyPress(
                ChaosKeyCode::Escape,
                true,
            )]);
            event_manager.get_commands()
        }

    #[test]
    fn cmd_is_returned_from_get_commands_when_event_matches_multi_key_press() {
        let mut event_manager = ChaosDeviceEventSystem::new();
        event_manager.register_multi_key_press::<TestCmd>(ChaosKeyCode::Escape, 2);

        let commands = event_manager.get(&vec![ChaosDeviceDetailedEvent::KeyPress(
            ChaosKeyCode::Escape,
            true,
        )]);

        assert!(commands.contains(&TypeId::of::<TestCmd>()));
    }*/
}
