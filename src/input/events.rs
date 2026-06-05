use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use winit::event::WindowEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

pub type ChaosKeyCode = KeyCode;

// Event to use in registration
#[derive(Hash, PartialEq, Eq, Copy, Clone)]
pub enum ChaosDeviceEventRegistration {
    CloseRequested,
    Focused,
    Unfocused,
    KeyPress(KeyCode), // only handles keyboard input for now (keycode and if it is pressed)
    MultiKeyPress(KeyCode, u8),
}

// This struct is to store the just what happened, just the basics and no details
#[derive(Debug, Eq, Clone)]
pub enum ChaosDeviceEvent {
    CloseRequested,
    Focused,
    Unfocused,
    KeyPress(KeyCode),
    Unrecognized,
}

// Event that is created from a winit event
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum ChaosDeviceDetailedEvent {
    CloseRequested,
    Focused,
    Unfocused,
    KeyPress(KeyCode, bool), // only handles keyboard input for now (keycode and if it is pressed)
    Unrecognized,
}

// key state storage (to store how often we have pressed etc)
#[derive(Clone)]
pub struct ChaosDeviceEventState {
    button: KeyCode,
    pressed: bool,
    last_release_time: Instant,
    last_pressed_time: Instant,
    actual_repeats: u8,
}

impl From<ChaosDeviceDetailedEvent> for ChaosDeviceEvent {
    fn from(detailed: ChaosDeviceDetailedEvent) -> Self {
        match detailed {
            ChaosDeviceDetailedEvent::KeyPress(code, _) => ChaosDeviceEvent::KeyPress(code),
            ChaosDeviceDetailedEvent::Focused => ChaosDeviceEvent::Focused,
            ChaosDeviceDetailedEvent::CloseRequested => ChaosDeviceEvent::CloseRequested,
            ChaosDeviceDetailedEvent::Unfocused => ChaosDeviceEvent::Unfocused,
            ChaosDeviceDetailedEvent::Unrecognized => ChaosDeviceEvent::Unrecognized,
        }
    }
}

impl From<&ChaosDeviceDetailedEvent> for ChaosDeviceEvent {
    fn from(detailed: &ChaosDeviceDetailedEvent) -> Self {
        match detailed {
            ChaosDeviceDetailedEvent::KeyPress(code, _) => ChaosDeviceEvent::KeyPress(*code),
            ChaosDeviceDetailedEvent::Focused => ChaosDeviceEvent::Focused,
            ChaosDeviceDetailedEvent::CloseRequested => ChaosDeviceEvent::CloseRequested,
            ChaosDeviceDetailedEvent::Unfocused => ChaosDeviceEvent::Unfocused,
            ChaosDeviceDetailedEvent::Unrecognized => ChaosDeviceEvent::Unrecognized,
        }
    }
}

impl PartialEq for ChaosDeviceEvent {
    fn eq(&self, other: &ChaosDeviceEvent) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl Hash for ChaosDeviceEvent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

// Converter for winit::Event to ChaosDeviceEvent
impl From<&WindowEvent> for ChaosDeviceDetailedEvent {
    fn from(event: &WindowEvent) -> Self {
        match event {
            WindowEvent::CloseRequested => ChaosDeviceDetailedEvent::CloseRequested,
            WindowEvent::Focused(focused) => {
                if *focused {
                    ChaosDeviceDetailedEvent::Focused
                } else {
                    ChaosDeviceDetailedEvent::Unfocused
                }
            }
            WindowEvent::KeyboardInput { event, .. } => match event.physical_key {
                PhysicalKey::Unidentified(_) => {
                    ChaosDeviceDetailedEvent::Unrecognized
                }
                PhysicalKey::Code(key) => {
                    ChaosDeviceDetailedEvent::KeyPress(key, event.state.is_pressed())
                }
            },
            _ => ChaosDeviceDetailedEvent::Unrecognized,
        }
    }
}

impl From<ChaosDeviceEventRegistration> for ChaosDeviceDetailedEvent {
    fn from(cder: ChaosDeviceEventRegistration) -> Self {
        match cder {
            ChaosDeviceEventRegistration::CloseRequested => {
                ChaosDeviceDetailedEvent::CloseRequested
            }
            ChaosDeviceEventRegistration::KeyPress(key) => {
                ChaosDeviceDetailedEvent::KeyPress(key, true)
            }
            ChaosDeviceEventRegistration::MultiKeyPress(key, _) => {
                ChaosDeviceDetailedEvent::KeyPress(key, true)
            }
            ChaosDeviceEventRegistration::Focused => ChaosDeviceDetailedEvent::Focused,
            ChaosDeviceEventRegistration::Unfocused => ChaosDeviceDetailedEvent::Unfocused,
        }
    }
}

impl PartialEq<ChaosDeviceDetailedEvent> for ChaosDeviceEventRegistration {
    fn eq(&self, other: &ChaosDeviceDetailedEvent) -> bool {
        // Only makes sure that the ChaosDeviceEvent is mapped to the same key as the ChaoInputEventRegistration
        match other {
            ChaosDeviceDetailedEvent::KeyPress(input_key, _) => match self {
                ChaosDeviceEventRegistration::KeyPress(registration_input_key) => {
                    input_key == registration_input_key
                }
                _ => false,
            },
            ChaosDeviceDetailedEvent::Unfocused => *self == ChaosDeviceEventRegistration::Unfocused,
            ChaosDeviceDetailedEvent::Focused => *self == ChaosDeviceEventRegistration::Focused,
            ChaosDeviceDetailedEvent::CloseRequested => {
                *self == ChaosDeviceEventRegistration::CloseRequested
            }
            _ => false,
        }
    }
}

impl Display for ChaosDeviceEventRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChaosDeviceEventRegistration::CloseRequested => write!(f, "CloseRequested"),
            ChaosDeviceEventRegistration::Focused => write!(f, "Focused"),
            ChaosDeviceEventRegistration::Unfocused => write!(f, "Unfocused"),
            ChaosDeviceEventRegistration::KeyPress(key) => write!(f, "KeyPress({:?})", key),
            ChaosDeviceEventRegistration::MultiKeyPress(key, repeats) => {
                write!(f, "MultiKeyPress({:?}, {})", key, repeats)
            }
        }
    }
}

impl ChaosDeviceEventState {
    pub fn new_single(key: KeyCode) -> ChaosDeviceEventState {
        ChaosDeviceEventState {
            pressed: false,
            last_pressed_time: Instant::now(),
            last_release_time: Instant::now(),
            button: key,
            actual_repeats: 0,
        }
    }

    pub fn new_multi(key: KeyCode, _repeats: u8) -> ChaosDeviceEventState {
        ChaosDeviceEventState {
            pressed: false,
            last_pressed_time: Instant::now(),
            last_release_time: Instant::now(),
            button: key,
            actual_repeats: 0,
        }
    }

    pub fn update(
        &mut self,
        device_event: &ChaosDeviceDetailedEvent,
        multi_press_time_delta_millis: u128,
    ) {
        if let ChaosDeviceDetailedEvent::KeyPress(event_key_code, event_pressed) = device_event
            && *event_key_code == self.button
        {
            let now = Instant::now();
            if *event_pressed {
                // check if we are multi pressing
                if now.duration_since(self.last_pressed_time).as_millis()
                    < multi_press_time_delta_millis
                {
                    self.actual_repeats += 1;
                } else {
                    // reset the repeats if we timed out
                    self.actual_repeats = 1;
                }
                self.last_release_time = now;
            } else {
                self.last_pressed_time = now;
            }

            self.pressed = !self.pressed;
        }
    }
}

impl PartialEq for ChaosDeviceEventState {
    fn eq(&self, other: &ChaosDeviceEventState) -> bool {
        self.button == other.button
    }
}

impl PartialEq<ChaosDeviceEventRegistration> for ChaosDeviceEventState {
    fn eq(&self, other: &ChaosDeviceEventRegistration) -> bool {
        match other {
            ChaosDeviceEventRegistration::KeyPress(key_code) => {
                self.button == *key_code && self.pressed
            }
            ChaosDeviceEventRegistration::MultiKeyPress(key_code, repeats) => {
                self.button == *key_code && self.pressed && self.actual_repeats % repeats == 0
            }
            _ => false,
        }
    }
}
