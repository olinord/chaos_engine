use std::time::{Duration, Instant};

use winit::event::WindowEvent;

use crate::device::{
    events::{ChaosDeviceEvent, ChaosInputEvent, ChaosKeyCode, ChaosMouseButton},
    system::ChaosBindingContext,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChaosBindingEvent {
    Input(ChaosInputEventMatcher),
    Device(ChaosDeviceEventMatcher),
    Sequence {
        events: Vec<ChaosInputEventMatcher>,
        within: Duration,
    },
    Held {
        button: ChaosButton,
        duration: Duration,
        continue_after_matching: bool,
    },
    Chord {
        keys: Vec<ChaosButton>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChaosButton {
    Mouse(ChaosMouseButton),
    Keyboard(ChaosKeyCode),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChaosInputEventMatcher {
    Pressed(ChaosButton),
    Released(ChaosButton),
    MouseMoved,
    MouseWheel,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChaosDeviceEventMatcher {
    CloseRequested,
    Focused,
    Unfocused,
    Resized,
    Moved,
    MouseEntered,
    MouseExited,
}

impl ChaosBindingEvent {
    pub fn matches(&self, context: &ChaosBindingContext, event: &WindowEvent) -> bool {
        self.matches_at(context, event, Instant::now())
    }

    pub(crate) fn matches_at(
        &self,
        context: &ChaosBindingContext,
        event: &WindowEvent,
        now: Instant,
    ) -> bool {
        match self {
            ChaosBindingEvent::Input(input_event) => input_event.matches(context, event),
            ChaosBindingEvent::Device(device_event) => device_event.matches(event),
            ChaosBindingEvent::Sequence { events, within } => {
                ChaosInputEvent::try_from(event).is_ok()
                    && context.sequence_matches_at(events, *within, now)
            }
            ChaosBindingEvent::Held {
                button, duration, ..
            } => context.button_held_for_at(button, *duration, now),
            ChaosBindingEvent::Chord { keys } => context.chord_matches(keys),
        }
    }
}

impl ChaosInputEventMatcher {
    pub fn matches(&self, _context: &ChaosBindingContext, windows_event: &WindowEvent) -> bool {
        ChaosInputEvent::try_from(windows_event)
            .map(|input_event| self.matches_input_event(&input_event))
            .unwrap_or(false)
    }

    pub(crate) fn matches_input_event(&self, input_event: &ChaosInputEvent) -> bool {
        match self {
            ChaosInputEventMatcher::Pressed(button) => {
                Self::button_matches(button, input_event, true)
            }
            ChaosInputEventMatcher::Released(button) => {
                Self::button_matches(button, input_event, false)
            }
            ChaosInputEventMatcher::MouseMoved => {
                matches!(input_event, ChaosInputEvent::MousePosition { .. })
            }
            ChaosInputEventMatcher::MouseWheel => {
                matches!(input_event, ChaosInputEvent::MouseWheel { .. })
            }
        }
    }

    fn button_matches(
        button: &ChaosButton,
        input_event: &ChaosInputEvent,
        expected_pressed: bool,
    ) -> bool {
        match (button, input_event) {
            (
                ChaosButton::Keyboard(expected_keycode),
                ChaosInputEvent::KeyboardInput { keycode, pressed },
            ) => expected_keycode == keycode && *pressed == expected_pressed,
            (
                ChaosButton::Mouse(expected_button),
                ChaosInputEvent::MouseButton { button, pressed },
            ) => expected_button == button && *pressed == expected_pressed,
            _ => false,
        }
    }
}

impl ChaosDeviceEventMatcher {
    pub fn matches(&self, windows_event: &WindowEvent) -> bool {
        ChaosDeviceEvent::try_from(windows_event)
            .map(|device_event| self.matches_device_event(&device_event))
            .unwrap_or(false)
    }

    pub(crate) fn matches_device_event(&self, device_event: &ChaosDeviceEvent) -> bool {
        matches!(
            (self, device_event),
            (
                ChaosDeviceEventMatcher::CloseRequested,
                ChaosDeviceEvent::CloseRequested
            ) | (ChaosDeviceEventMatcher::Focused, ChaosDeviceEvent::Focused)
                | (
                    ChaosDeviceEventMatcher::Unfocused,
                    ChaosDeviceEvent::Unfocused
                )
                | (
                    ChaosDeviceEventMatcher::Resized,
                    ChaosDeviceEvent::Resized(_, _)
                )
                | (
                    ChaosDeviceEventMatcher::Moved,
                    ChaosDeviceEvent::Moved(_, _)
                )
                | (
                    ChaosDeviceEventMatcher::MouseEntered,
                    ChaosDeviceEvent::MouseEntered
                )
                | (
                    ChaosDeviceEventMatcher::MouseExited,
                    ChaosDeviceEvent::MouseExited
                )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};

    #[test]
    fn input_matcher_matches_mouse_button_window_event() {
        let context = ChaosBindingContext::new();
        let event = WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Left,
        };

        assert!(
            ChaosInputEventMatcher::Pressed(ChaosButton::Mouse(MouseButton::Left))
                .matches(&context, &event)
        );
        assert!(
            !ChaosInputEventMatcher::Released(ChaosButton::Mouse(MouseButton::Left))
                .matches(&context, &event)
        );
    }

    #[test]
    fn device_matcher_matches_close_requested_window_event() {
        assert!(ChaosDeviceEventMatcher::CloseRequested.matches(&WindowEvent::CloseRequested));
        assert!(!ChaosDeviceEventMatcher::Focused.matches(&WindowEvent::CloseRequested));
    }

    #[test]
    fn binding_event_matches_input_and_device_window_events() {
        let context = ChaosBindingContext::new();
        let mouse_event = WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Released,
            button: MouseButton::Right,
        };

        assert!(
            ChaosBindingEvent::Input(ChaosInputEventMatcher::Released(ChaosButton::Mouse(
                MouseButton::Right
            ),))
            .matches(&context, &mouse_event)
        );
        assert!(
            ChaosBindingEvent::Device(ChaosDeviceEventMatcher::CloseRequested)
                .matches(&context, &WindowEvent::CloseRequested)
        );
    }
}
