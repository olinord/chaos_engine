use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use winit::event::WindowEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

pub type ChaosKeyCode = KeyCode;

// Event to use in registration
#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
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
#[derive(Clone, Debug)]
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
        match (self, other) {
            (Self::CloseRequested, Self::CloseRequested)
            | (Self::Focused, Self::Focused)
            | (Self::Unfocused, Self::Unfocused)
            | (Self::Unrecognized, Self::Unrecognized) => true,
            (Self::KeyPress(press), Self::KeyPress(other_press)) => press == other_press,
            _ => false,
        }
    }
}

impl Hash for ChaosDeviceEvent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);

        if let Self::KeyPress(key) = self {
            key.hash(state);
        }
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
                PhysicalKey::Unidentified(_) => ChaosDeviceDetailedEvent::Unrecognized,
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
                self.button == *key_code && self.pressed && self.actual_repeats == *repeats
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_event(event: &ChaosDeviceEvent) -> u64 {
        let mut hasher = DefaultHasher::new();
        event.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_chaos_device_event_equality() {
        let e1 = ChaosDeviceEvent::Unfocused;
        let e2 = ChaosDeviceEvent::Unfocused;
        assert_eq!(e1, e2);
    }

    #[test]
    fn detailed_key_press_converts_to_basic_key_press_without_pressed_state() {
        let pressed = ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true);
        let released = ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, false);

        assert_eq!(
            ChaosDeviceEvent::from(pressed),
            ChaosDeviceEvent::KeyPress(KeyCode::Escape)
        );
        assert_eq!(
            ChaosDeviceEvent::from(&released),
            ChaosDeviceEvent::KeyPress(KeyCode::Escape)
        );
    }

    #[test]
    fn different_key_presses_are_not_equal_or_hash_equivalent() {
        let escape = ChaosDeviceEvent::KeyPress(KeyCode::Escape);
        let space = ChaosDeviceEvent::KeyPress(KeyCode::Space);

        assert_ne!(escape, space);
        assert_ne!(hash_event(&escape), hash_event(&space));
    }

    #[test]
    fn registration_converts_to_pressed_detailed_event() {
        assert_eq!(
            ChaosDeviceDetailedEvent::from(ChaosDeviceEventRegistration::KeyPress(KeyCode::Escape)),
            ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true)
        );
        assert_eq!(
            ChaosDeviceDetailedEvent::from(ChaosDeviceEventRegistration::MultiKeyPress(
                KeyCode::Space,
                2,
            )),
            ChaosDeviceDetailedEvent::KeyPress(KeyCode::Space, true)
        );
    }

    #[test]
    fn single_key_registration_matches_press_and_release_for_same_key() {
        let registration = ChaosDeviceEventRegistration::KeyPress(KeyCode::Escape);

        assert_eq!(
            registration,
            ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true)
        );
        assert_eq!(
            registration,
            ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, false)
        );
        assert_ne!(
            registration,
            ChaosDeviceDetailedEvent::KeyPress(KeyCode::Space, true)
        );
    }

    #[test]
    fn registration_display_includes_variant_and_key_details() {
        assert_eq!(
            ChaosDeviceEventRegistration::CloseRequested.to_string(),
            "CloseRequested"
        );
        assert_eq!(
            ChaosDeviceEventRegistration::KeyPress(KeyCode::Escape).to_string(),
            "KeyPress(Escape)"
        );
        assert_eq!(
            ChaosDeviceEventRegistration::MultiKeyPress(KeyCode::Space, 2).to_string(),
            "MultiKeyPress(Space, 2)"
        );
    }

    #[test]
    fn event_state_matches_single_registration_only_while_pressed() {
        let mut state = ChaosDeviceEventState::new_single(KeyCode::Escape);
        let registration = ChaosDeviceEventRegistration::KeyPress(KeyCode::Escape);

        assert_ne!(state, registration);

        state.update(
            &ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true),
            300,
        );
        assert_eq!(state, registration);

        state.update(
            &ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, false),
            300,
        );
        assert_ne!(state, registration);
    }

    #[test]
    fn event_state_counts_multi_press_repeats_for_same_key() {
        let mut state = ChaosDeviceEventState::new_multi(KeyCode::Escape, 2);
        let registration = ChaosDeviceEventRegistration::MultiKeyPress(KeyCode::Escape, 2);

        state.update(
            &ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true),
            300,
        );
        assert_ne!(state, registration);

        state.update(
            &ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, false),
            300,
        );
        state.update(
            &ChaosDeviceDetailedEvent::KeyPress(KeyCode::Escape, true),
            300,
        );
        assert_eq!(state, registration);
    }

    #[test]
    fn event_state_ignores_other_keys() {
        let mut state = ChaosDeviceEventState::new_single(KeyCode::Escape);
        let registration = ChaosDeviceEventRegistration::KeyPress(KeyCode::Escape);

        state.update(
            &ChaosDeviceDetailedEvent::KeyPress(KeyCode::Space, true),
            300,
        );

        assert_ne!(state, registration);
    }
}
