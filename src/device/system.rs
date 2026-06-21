use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    time::{Duration, Instant},
};

use chaos_communicator::message::{ChaosMessage, ChaosMessageBuilder};
use winit::event::WindowEvent;

use crate::{
    device::{
        bindings::{ChaosBindingEvent, ChaosButton, ChaosInputEventMatcher},
        events::{ChaosDeviceEvent, ChaosInputEvent},
    },
    triggers::trigger_event_key::TriggerEventKey,
};

const MAX_INPUT_HISTORY: usize = 64;

pub struct ChaosBindingContext {
    // Tracks the currently pressed buttons (keyboard keys and mouse buttons)
    pressed_buttons: HashSet<ChaosButton>,
    // Tracks the time when each button was pressed
    held_since: HashMap<ChaosButton, Instant>,
    // Tracks which held bindings have already been fired to prevent repeated firing
    fired_held_bindings: HashSet<(ChaosButton, Duration)>,
    // Stores a history of input events with their timestamps for sequence matching
    input_history: VecDeque<(Instant, ChaosInputEvent)>,
}

impl ChaosBindingContext {
    pub fn new() -> Self {
        Self {
            pressed_buttons: HashSet::new(),
            held_since: HashMap::new(),
            fired_held_bindings: HashSet::new(),
            input_history: VecDeque::new(),
        }
    }

    pub(crate) fn button_held_for_at(
        &self,
        key: &ChaosButton,
        duration: Duration,
        now: Instant,
    ) -> bool {
        self.held_since
            .get(key)
            .map(|pressed_at| now.duration_since(*pressed_at) >= duration)
            .unwrap_or(false)
    }

    pub(crate) fn held_binding_matches(
        &mut self,
        button: &ChaosButton,
        duration: Duration,
        now: Instant,
        continue_after_matching: bool,
    ) -> bool {
        if !self.button_held_for_at(button, duration, now) {
            return false;
        }

        if continue_after_matching {
            true
        } else {
            self.fired_held_bindings.insert((button.clone(), duration))
        }
    }

    pub(crate) fn chord_matches(&self, keys: &[ChaosButton]) -> bool {
        !keys.is_empty() && keys.iter().all(|key| self.pressed_buttons.contains(key))
    }

    pub(crate) fn sequence_matches_at(
        &self,
        events: &[ChaosInputEventMatcher],
        within: Duration,
        now: Instant,
    ) -> bool {
        if events.is_empty() {
            return false;
        }

        let recent_events = self
            .input_history
            .iter()
            .filter(|(recorded_at, _)| now.duration_since(*recorded_at) <= within)
            .map(|(_, event)| event)
            .collect::<Vec<_>>();

        if recent_events.len() < events.len() {
            return false;
        }

        recent_events[recent_events.len() - events.len()..]
            .iter()
            .zip(events.iter())
            .all(|(input_event, matcher)| matcher.matches_input_event(input_event))
    }
}

pub struct DeviceEventSystem {
    bindings: Vec<BoundSignal>,
    context: ChaosBindingContext,
}

struct BoundSignal {
    binding: ChaosBindingEvent,
    build_message: Box<dyn Fn() -> ChaosMessage>,
}

impl DeviceEventSystem {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            context: ChaosBindingContext::new(),
        }
    }

    pub fn bind<T>(&mut self, binding: ChaosBindingEvent, signal: T) -> TriggerEventKey
    where
        T: Any + Hash + Clone + Send + Sync + 'static,
    {
        let signal_key = TriggerEventKey::new(&signal);
        let signal_payload = signal.clone();

        self.bindings.push(BoundSignal {
            binding,
            build_message: Box::new(move || {
                ChaosMessageBuilder::new()
                    .with_param("signal", signal_payload.clone())
                    .build_for_event(signal_key)
            }),
        });

        signal_key
    }

    pub fn update(&mut self, event: &WindowEvent) -> Vec<ChaosMessage> {
        self.update_with_chaos_events(
            ChaosInputEvent::try_from(event).ok(),
            ChaosDeviceEvent::try_from(event).ok(),
            Instant::now(),
        )
    }

    fn update_with_chaos_events(
        &mut self,
        input_event: Option<ChaosInputEvent>,
        device_event: Option<ChaosDeviceEvent>,
        now: Instant,
    ) -> Vec<ChaosMessage> {
        let input_event =
            input_event.and_then(|input_event| self.update_input_state(input_event, now));

        let context = &mut self.context;

        self.bindings
            .iter()
            .filter(|bound_signal| {
                Self::binding_matches(
                    context,
                    &bound_signal.binding,
                    input_event.as_ref(),
                    device_event.as_ref(),
                    now,
                )
            })
            .map(|bound_signal| (bound_signal.build_message)())
            .collect()
    }

    fn update_input_state(
        &mut self,
        input_event: ChaosInputEvent,
        now: Instant,
    ) -> Option<ChaosInputEvent> {
        let state_changed = match &input_event {
            ChaosInputEvent::KeyboardInput { keycode, pressed } => {
                self.update_button_state(ChaosButton::Keyboard(keycode.clone()), *pressed, now)
            }
            ChaosInputEvent::MouseButton { button, pressed } => {
                self.update_button_state(ChaosButton::Mouse(button.clone()), *pressed, now)
            }
            _ => true,
        };

        if !state_changed {
            return None;
        }

        self.context
            .input_history
            .push_back((now, input_event.clone()));
        while self.context.input_history.len() > MAX_INPUT_HISTORY {
            self.context.input_history.pop_front();
        }

        Some(input_event)
    }

    fn update_button_state(&mut self, button: ChaosButton, pressed: bool, now: Instant) -> bool {
        if pressed {
            if self.context.pressed_buttons.contains(&button) {
                return false;
            }
            self.context.held_since.insert(button.clone(), now);
            self.context.pressed_buttons.insert(button);
            true
        } else {
            if !self.context.pressed_buttons.remove(&button) {
                return false;
            }
            self.context.held_since.remove(&button);
            self.context
                .fired_held_bindings
                .retain(|(held_button, _)| held_button != &button);
            true
        }
    }

    fn binding_matches(
        context: &mut ChaosBindingContext,
        binding: &ChaosBindingEvent,
        input_event: Option<&ChaosInputEvent>,
        device_event: Option<&ChaosDeviceEvent>,
        now: Instant,
    ) -> bool {
        match binding {
            ChaosBindingEvent::Input(matcher) => input_event
                .map(|input_event| matcher.matches_input_event(input_event))
                .unwrap_or(false),
            ChaosBindingEvent::Device(matcher) => device_event
                .map(|device_event| matcher.matches_device_event(device_event))
                .unwrap_or(false),
            ChaosBindingEvent::Sequence { events, within } => {
                input_event.is_some() && context.sequence_matches_at(events, *within, now)
            }
            ChaosBindingEvent::Held {
                button,
                duration,
                continue_after_matching,
            } => context.held_binding_matches(button, *duration, now, *continue_after_matching),
            ChaosBindingEvent::Chord { keys } => {
                input_event.is_some() && context.chord_matches(keys)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::bindings::ChaosDeviceEventMatcher;
    use winit::keyboard::KeyCode;

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum TestSignal {
        Fire,
        Close,
    }

    #[test]
    fn bind_returns_trigger_key_for_signal() {
        let mut system = DeviceEventSystem::new();

        let signal_key = system.bind(
            ChaosBindingEvent::Input(ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(
                KeyCode::Space,
            ))),
            TestSignal::Fire,
        );

        assert_eq!(signal_key, TriggerEventKey::new(&TestSignal::Fire));
    }

    #[test]
    fn input_binding_emits_signal_message_when_event_matches() {
        let mut system = DeviceEventSystem::new();
        let signal_key = system.bind(
            ChaosBindingEvent::Input(ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(
                KeyCode::Space,
            ))),
            TestSignal::Fire,
        );

        let messages = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            Instant::now(),
        );

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].get_event(),
            ChaosMessageBuilder::new()
                .build_for_event(signal_key)
                .get_event()
        );
        assert_eq!(
            messages[0].get::<TestSignal>("signal"),
            Some(TestSignal::Fire)
        );
    }

    #[test]
    fn pressed_input_binding_only_emits_once_until_released() {
        let mut system = DeviceEventSystem::new();
        system.bind(
            ChaosBindingEvent::Input(ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(
                KeyCode::Space,
            ))),
            TestSignal::Fire,
        );
        let now = Instant::now();

        let first_press = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            now,
        );
        let repeated_press = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(16),
        );
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: false,
            }),
            None,
            now + Duration::from_millis(32),
        );
        let press_after_release = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(48),
        );

        assert_eq!(first_press.len(), 1);
        assert!(repeated_press.is_empty());
        assert_eq!(press_after_release.len(), 1);
    }

    #[test]
    fn held_binding_can_emit_once_until_released() {
        let mut system = DeviceEventSystem::new();
        system.bind(
            ChaosBindingEvent::Held {
                button: ChaosButton::Keyboard(KeyCode::Space),
                duration: Duration::from_millis(10),
                continue_after_matching: false,
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        let initial_press = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            now,
        );
        let first_held_match =
            system.update_with_chaos_events(None, None, now + Duration::from_millis(10));
        let repeated_held_match =
            system.update_with_chaos_events(None, None, now + Duration::from_millis(20));
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: false,
            }),
            None,
            now + Duration::from_millis(30),
        );
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(40),
        );
        let held_match_after_release =
            system.update_with_chaos_events(None, None, now + Duration::from_millis(50));

        assert!(initial_press.is_empty());
        assert_eq!(first_held_match.len(), 1);
        assert!(repeated_held_match.is_empty());
        assert_eq!(held_match_after_release.len(), 1);
    }

    #[test]
    fn held_binding_can_emit_once_continuously_until_released() {
        let mut system = DeviceEventSystem::new();
        system.bind(
            ChaosBindingEvent::Held {
                button: ChaosButton::Keyboard(KeyCode::Space),
                duration: Duration::from_millis(10),
                continue_after_matching: true,
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        let initial_press = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            now,
        );
        let first_held_match =
            system.update_with_chaos_events(None, None, now + Duration::from_millis(10));
        let repeated_held_match =
            system.update_with_chaos_events(None, None, now + Duration::from_millis(20));
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: false,
            }),
            None,
            now + Duration::from_millis(30),
        );
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(40),
        );
        let held_match_after_release =
            system.update_with_chaos_events(None, None, now + Duration::from_millis(50));

        assert!(initial_press.is_empty());
        assert_eq!(first_held_match.len(), 1);
        assert_eq!(repeated_held_match.len(), 1);
        assert_eq!(held_match_after_release.len(), 1);
    }

    #[test]
    fn device_binding_emits_signal_message_when_event_matches() {
        let mut system = DeviceEventSystem::new();
        system.bind(
            ChaosBindingEvent::Device(ChaosDeviceEventMatcher::CloseRequested),
            TestSignal::Close,
        );

        let messages = system.update_with_chaos_events(
            None,
            Some(ChaosDeviceEvent::CloseRequested),
            Instant::now(),
        );

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].get::<TestSignal>("signal"),
            Some(TestSignal::Close)
        );
    }
}
