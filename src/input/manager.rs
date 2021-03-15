use std::collections::HashMap;
use commands::cmd::{Cmd, ExitCmd};
use winit::{Event, VirtualKeyCode};
use input::events::{ChaosDeviceEventState, ChaosDeviceEvent};
use input::events::ChaosDeviceEventRegistration;
use std::any::TypeId;


pub struct ChaosDeviceEventManager {
    registered_commands: HashMap<ChaosDeviceEventRegistration, TypeId>,
    registered_events: HashMap<ChaosDeviceEvent, (ChaosDeviceEventRegistration, Option<ChaosDeviceEventState>)>,
}

impl ChaosDeviceEventManager {
    pub fn new() -> ChaosDeviceEventManager {
        let mut cdem =  ChaosDeviceEventManager {
            registered_commands: HashMap::new(),
            registered_events: HashMap::new()
        };

        // Default event registrations.
        // Need to know when to close the damn thing
        cdem.register_command::<ExitCmd>(ChaosDeviceEventRegistration::CloseRequested);

        return cdem;
    }

    pub fn register_single_key_press<T: Cmd + 'static>(&mut self, input_key: VirtualKeyCode) {
        self.registered_commands.insert(ChaosDeviceEventRegistration::KeyPress(input_key), TypeId::of::<T>());
        self.registered_events.insert(ChaosDeviceEvent::KeyPress(input_key, true),
                                      (
                                               ChaosDeviceEventRegistration::KeyPress(input_key),
                                               Some(ChaosDeviceEventState::new_single( input_key ))
                                           )
                                       );
    }

    pub fn register_multi_key_press<T: Cmd + 'static>(&mut self, input_key: VirtualKeyCode, repeats: u128) {
        self.registered_commands.insert(ChaosDeviceEventRegistration::MultiKeyPress(input_key, repeats), TypeId::of::<T>());
        self.registered_events.insert(ChaosDeviceEvent::KeyPress(input_key, true),
                                      (
                                           ChaosDeviceEventRegistration::MultiKeyPress(input_key, repeats),
                                           Some(ChaosDeviceEventState::new_multi( input_key, repeats))
                                       )
        );    }

    pub fn register_command<T: Cmd + 'static>(&mut self, event: ChaosDeviceEventRegistration) {
        self.registered_commands.insert(event, TypeId::of::<T>());
        self.registered_events.insert(ChaosDeviceEvent::from(event), (event.clone(), None) );
    }

    pub fn get_commands(&mut self, events: &Vec<Event>) -> Vec<TypeId> {
        let mut commands = Vec::new();
        for event in events {
            let chaos_device_event = ChaosDeviceEvent::from(event);
            match self.registered_events.get_mut(&chaos_device_event) {
                Some((registration, state)) => {
                    match state {
                        Some(button_state) => {
                            // If this is a button specific event, handle that here
                            button_state.update(&chaos_device_event, 100);

                            // We can do this because of the partial eq impl for ChaosDeviceEventRegistration
                            if button_state == registration {
                                if let Some(command) = self.registered_commands.get(registration) {
                                    commands.push(command.clone() );
                                }
                            }
                        },
                        None => {
                            // Maybe this was a non button related event
                            // like close window, resize etc
                            if let Some(command) = self.registered_commands.get(registration) {
                                commands.push(command.clone());
                            }
                        }
                    }
                },
                None => ()
            };
        }
        return commands;
    }
}

