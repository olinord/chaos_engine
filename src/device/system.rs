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

/// Opaque handle to a registered binding, used with [`DeviceEventSystem::unbind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingId(u64);

/// Returned by [`DeviceEventSystem::bind`]. `trigger_key` identifies the signal event
/// (subscribers listen on this), `id` identifies the binding instance (for `unbind`).
#[derive(Debug, Clone, Copy)]
pub struct BindingHandle {
    pub trigger_key: TriggerEventKey,
    pub id: BindingId,
}

impl From<BindingHandle> for TriggerEventKey {
    fn from(handle: BindingHandle) -> Self {
        handle.trigger_key
    }
}

pub struct ChaosBindingContext {
    // Tracks the currently pressed buttons (keyboard keys and mouse buttons).
    pressed_buttons: HashSet<ChaosButton>,
    // Tracks the time when each button was pressed.
    held_since: HashMap<ChaosButton, Instant>,
    // Held bindings already fired since their most recent press (fire-once semantics).
    fired_held_bindings: HashSet<(ChaosButton, Duration)>,
    // Chord bindings already fired since one of their keys was last released.
    fired_chord_bindings: HashSet<Vec<ChaosButton>>,
    // Input events retained for sequence matching. Only events referenced by a Sequence
    // binding are pushed here, so mouse motion and other high-frequency events never
    // evict older button presses.
    input_history: VecDeque<(Instant, ChaosInputEvent)>,
}

impl Default for ChaosBindingContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ChaosBindingContext {
    pub(crate) fn new() -> Self {
        Self {
            pressed_buttons: HashSet::new(),
            held_since: HashMap::new(),
            fired_held_bindings: HashSet::new(),
            fired_chord_bindings: HashSet::new(),
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
        continuous: bool,
    ) -> bool {
        if !self.button_held_for_at(button, duration, now) {
            return false;
        }

        if continuous {
            true
        } else {
            self.fired_held_bindings.insert((button.clone(), duration))
        }
    }

    // Immutable peek: are all chord keys currently held? Used by `ChaosBindingEvent::matches`
    // for testing/inspection only; the dispatch path uses `chord_binding_matches` instead.
    pub(crate) fn chord_matches(&self, keys: &[ChaosButton]) -> bool {
        !keys.is_empty() && keys.iter().all(|key| self.pressed_buttons.contains(key))
    }

    // Fire-once chord match: the current input event must be a press of one of the chord
    // keys, all chord keys must be held, and the chord must not have fired yet since one
    // of its keys was last released.
    pub(crate) fn chord_binding_matches(
        &mut self,
        keys: &[ChaosButton],
        input_event: Option<&ChaosInputEvent>,
    ) -> bool {
        if keys.is_empty() {
            return false;
        }

        let triggered_by_chord_press = match input_event {
            Some(ChaosInputEvent::KeyboardInput {
                keycode,
                pressed: true,
            }) => keys
                .iter()
                .any(|k| matches!(k, ChaosButton::Keyboard(kc) if kc == keycode)),
            Some(ChaosInputEvent::MouseButton {
                button,
                pressed: true,
            }) => keys
                .iter()
                .any(|k| matches!(k, ChaosButton::Mouse(mb) if mb == button)),
            _ => false,
        };
        if !triggered_by_chord_press {
            return false;
        }

        if !keys.iter().all(|k| self.pressed_buttons.contains(k)) {
            return false;
        }

        self.fired_chord_bindings.insert(keys.to_vec())
    }

    // Immutable peek: is the sequence the tail of recent input history? Kept for
    // `ChaosBindingEvent::matches` compatibility; the dispatch path uses
    // `sequence_binding_matches` which allows intervening unrelated events.
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

    // Fire-once sequence match: the current input event must match the last matcher, and
    // the preceding matchers must appear as an ordered subsequence in `input_history`
    // within `within` of `now`. This tolerates intervening unrelated events (e.g. mouse
    // motion) and correctly handles patterns like [Pressed(A), Pressed(A)] where a
    // Release(A) sits between the two presses.
    pub(crate) fn sequence_binding_matches(
        &self,
        events: &[ChaosInputEventMatcher],
        within: Duration,
        now: Instant,
        input_event: Option<&ChaosInputEvent>,
    ) -> bool {
        let (last_matcher, preceding) = match events.split_last() {
            Some(parts) => parts,
            None => return false,
        };
        let Some(current) = input_event else {
            return false;
        };
        if !last_matcher.matches_input_event(current) {
            return false;
        }
        if preceding.is_empty() {
            return true;
        }

        let mut idx = 0usize;
        for (recorded_at, event) in self.input_history.iter() {
            if now.duration_since(*recorded_at) > within {
                continue;
            }
            if preceding[idx].matches_input_event(event) {
                idx += 1;
                if idx == preceding.len() {
                    return true;
                }
            }
        }
        false
    }

    // A binding-independent check for what should be recorded in input_history for
    // sequence matching purposes.
    fn record_input_event(&mut self, event: ChaosInputEvent, now: Instant) {
        self.input_history.push_back((now, event));
        while self.input_history.len() > MAX_INPUT_HISTORY {
            self.input_history.pop_front();
        }
    }
}

pub struct DeviceEventSystem {
    bindings: Vec<BoundSignal>,
    context: ChaosBindingContext,
    next_binding_id: u64,
}

struct BoundSignal {
    id: BindingId,
    binding: ChaosBindingEvent,
    // Constructs the signal message on demand. Parameters (including event-specific
    // ones like `width`/`height` for a resize) are materialized when the binding
    // matches, not when it was registered.
    build_message: Box<
        dyn Fn(Option<&ChaosInputEvent>, Option<&ChaosDeviceEvent>) -> ChaosMessage + Send + Sync,
    >,
}

impl Default for DeviceEventSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceEventSystem {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            context: ChaosBindingContext::new(),
            next_binding_id: 0,
        }
    }

    pub fn bind<T>(&mut self, binding: ChaosBindingEvent, signal: T) -> BindingHandle
    where
        T: Any + Hash + Clone + Send + Sync + 'static,
    {
        let trigger_key = TriggerEventKey::new(&signal);
        let build_message: Box<
            dyn Fn(Option<&ChaosInputEvent>, Option<&ChaosDeviceEvent>) -> ChaosMessage
                + Send
                + Sync,
        > = Box::new(move |input_event, device_event| {
            let mut builder = ChaosMessageBuilder::new().with_param("signal", signal.clone());
            if let Some(ie) = input_event {
                builder = ie.enrich_message(builder);
            }
            if let Some(de) = device_event {
                builder = de.enrich_message(builder);
            }
            builder.build_for_event(trigger_key)
        });

        let id = BindingId(self.next_binding_id);
        self.next_binding_id += 1;

        self.bindings.push(BoundSignal {
            id,
            binding,
            build_message,
        });

        BindingHandle { trigger_key, id }
    }

    /// Remove a previously registered binding. Returns `true` if the binding was found
    /// and removed.
    pub fn unbind(&mut self, id: BindingId) -> bool {
        if let Some(pos) = self.bindings.iter().position(|b| b.id == id) {
            self.bindings.remove(pos);
            true
        } else {
            false
        }
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

        let mut messages = Vec::new();
        let mut input_referenced = false;
        let mut device_referenced = false;

        for bound_signal in &self.bindings {
            if !self.context.matches_binding(
                &bound_signal.binding,
                input_event.as_ref(),
                device_event.as_ref(),
                now,
            ) {
                continue;
            }

            messages.push((bound_signal.build_message)(
                input_event.as_ref(),
                device_event.as_ref(),
            ));

            if let Some(ie) = input_event.as_ref() {
                if !input_referenced
                    && Self::binding_references_input_event(&bound_signal.binding, ie)
                {
                    input_referenced = true;
                }
            }
            if let Some(de) = device_event.as_ref() {
                if !device_referenced
                    && Self::binding_references_device_event(&bound_signal.binding, de)
                {
                    device_referenced = true;
                }
            }
        }

        if input_referenced {
            if let Some(ie) = input_event {
                messages.push(ie.into());
            }
        }
        if device_referenced {
            if let Some(de) = device_event {
                messages.push(de.into());
            }
        }

        messages
    }

    fn update_input_state(
        &mut self,
        input_event: ChaosInputEvent,
        now: Instant,
    ) -> Option<ChaosInputEvent> {
        let state_changed = match &input_event {
            ChaosInputEvent::KeyboardInput { keycode, pressed } => {
                self.update_button_state(ChaosButton::Keyboard(*keycode), *pressed, now)
            }
            ChaosInputEvent::MouseButton { button, pressed } => {
                self.update_button_state(ChaosButton::Mouse(*button), *pressed, now)
            }
            _ => true,
        };

        if !state_changed {
            return None;
        }

        if self.event_is_relevant_for_sequence_history(&input_event) {
            self.context.record_input_event(input_event.clone(), now);
        }

        Some(input_event)
    }

    fn event_is_relevant_for_sequence_history(&self, event: &ChaosInputEvent) -> bool {
        self.bindings
            .iter()
            .any(|bound_signal| match &bound_signal.binding {
                ChaosBindingEvent::Sequence { events, .. } => events
                    .iter()
                    .any(|matcher| matcher.matches_input_event(event)),
                _ => false,
            })
    }

    // Returns true when the button state actually changed (i.e. this is a fresh
    // press/release rather than an OS-level repeat). Callers rely on this to decide
    // whether to record the event and emit downstream messages.
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
            self.context
                .fired_chord_bindings
                .retain(|keys| !keys.contains(&button));
            true
        }
    }

    fn binding_references_input_event(
        binding: &ChaosBindingEvent,
        input_event: &ChaosInputEvent,
    ) -> bool {
        match binding {
            ChaosBindingEvent::Input(matcher) => matcher.matches_input_event(input_event),
            ChaosBindingEvent::Sequence { events, .. } => events
                .iter()
                .any(|matcher| matcher.matches_input_event(input_event)),
            ChaosBindingEvent::Held { button, .. } => match (button, input_event) {
                (
                    ChaosButton::Keyboard(expected_key),
                    ChaosInputEvent::KeyboardInput { keycode, .. },
                ) => expected_key == keycode,
                (
                    ChaosButton::Mouse(expected_button),
                    ChaosInputEvent::MouseButton { button, .. },
                ) => expected_button == button,
                _ => false,
            },
            ChaosBindingEvent::Chord { keys } => {
                keys.iter().any(|button| match (button, input_event) {
                    (
                        ChaosButton::Keyboard(expected_key),
                        ChaosInputEvent::KeyboardInput { keycode, .. },
                    ) => expected_key == keycode,
                    (
                        ChaosButton::Mouse(expected_button),
                        ChaosInputEvent::MouseButton { button, .. },
                    ) => expected_button == button,
                    _ => false,
                })
            }
            ChaosBindingEvent::Device(_) => false,
        }
    }

    fn binding_references_device_event(
        binding: &ChaosBindingEvent,
        device_event: &ChaosDeviceEvent,
    ) -> bool {
        match binding {
            ChaosBindingEvent::Device(matcher) => matcher.matches_device_event(device_event),
            _ => false,
        }
    }
}

impl ChaosBindingContext {
    fn matches_binding(
        &mut self,
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
                self.sequence_binding_matches(events, *within, now, input_event)
            }
            ChaosBindingEvent::Held {
                button,
                duration,
                // `continuous` fires on every `update_with_chaos_events` call after the
                // hold duration is reached; the effective rate is set by how often the
                // caller ticks the system.
                continuous,
            } => self.held_binding_matches(button, *duration, now, *continuous),
            ChaosBindingEvent::Chord { keys } => self.chord_binding_matches(keys, input_event),
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

    fn signal_event(key: TriggerEventKey) -> u64 {
        ChaosMessageBuilder::new().build_for_event(key).get_event()
    }

    #[test]
    fn bind_returns_handle_with_signal_trigger_key() {
        let mut system = DeviceEventSystem::new();

        let handle = system.bind(
            ChaosBindingEvent::Input(ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(
                KeyCode::Space,
            ))),
            TestSignal::Fire,
        );

        assert_eq!(handle.trigger_key, TriggerEventKey::new(&TestSignal::Fire));

        // The handle is usable end-to-end: the signal message dispatched by the system
        // carries the same trigger key.
        let messages = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            Instant::now(),
        );
        assert!(
            messages
                .iter()
                .any(|m| m.get_event() == signal_event(handle.trigger_key))
        );
    }

    #[test]
    fn input_binding_emits_signal_message_when_event_matches() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
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

        assert_eq!(messages.len(), 2);
        assert!(messages.iter().any(|message| {
            message.get_event()
                == ChaosMessageBuilder::new()
                    .build_for_event(handle.trigger_key)
                    .get_event()
                && message.get::<TestSignal>("signal") == Some(TestSignal::Fire)
        }));
        assert!(messages.iter().any(|message| {
            message.get::<ChaosInputEvent>("input_event")
                == Some(ChaosInputEvent::KeyboardInput {
                    keycode: KeyCode::Space,
                    pressed: true,
                })
                && message.get::<bool>("pressed") == Some(true)
        }));
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

        assert_eq!(first_press.len(), 2);
        assert!(repeated_press.is_empty());
        assert_eq!(press_after_release.len(), 2);
    }

    #[test]
    fn held_binding_can_emit_once_until_released() {
        let mut system = DeviceEventSystem::new();
        system.bind(
            ChaosBindingEvent::Held {
                button: ChaosButton::Keyboard(KeyCode::Space),
                duration: Duration::from_millis(10),
                continuous: false,
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

        // Held bindings don't match on the initial press (duration not yet elapsed),
        // so no message is emitted for that event.
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
                continuous: true,
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

        // Held bindings don't match on the initial press (duration not yet elapsed),
        // so no message is emitted for that event.
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

        assert_eq!(messages.len(), 2);
        assert!(
            messages
                .iter()
                .any(|message| message.get::<TestSignal>("signal") == Some(TestSignal::Close))
        );
    }

    #[test]
    fn resized_device_event_produces_no_messages_when_unbound() {
        let mut system = DeviceEventSystem::new();

        let messages = system.update_with_chaos_events(
            None,
            Some(ChaosDeviceEvent::Resized(1280, 720)),
            Instant::now(),
        );

        assert!(messages.is_empty());
    }

    #[test]
    fn resized_device_event_emits_raw_payload_message_when_bound() {
        let mut system = DeviceEventSystem::new();
        system.bind(
            ChaosBindingEvent::Device(ChaosDeviceEventMatcher::Resized),
            TestSignal::Close,
        );

        let messages = system.update_with_chaos_events(
            None,
            Some(ChaosDeviceEvent::Resized(1280, 720)),
            Instant::now(),
        );

        assert_eq!(messages.len(), 2);
        assert!(
            messages
                .iter()
                .any(|message| { message.get::<TestSignal>("signal") == Some(TestSignal::Close) })
        );
        assert!(messages.iter().any(|message| {
            message.get_event()
                == ChaosMessageBuilder::new()
                    .build_for_event(ChaosDeviceEventMatcher::Resized)
                    .get_event()
                && message.get::<ChaosDeviceEvent>("device_event")
                    == Some(ChaosDeviceEvent::Resized(1280, 720))
                && message.get::<u32>("width") == Some(1280)
                && message.get::<u32>("height") == Some(720)
        }));
    }

    #[test]
    fn sequence_binding_fires_when_completed_by_current_event() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Sequence {
                events: vec![
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyA)),
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyB)),
                ],
                within: Duration::from_millis(500),
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        let after_a = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyA,
                pressed: true,
            }),
            None,
            now,
        );
        assert!(
            after_a
                .iter()
                .all(|m| m.get_event() != signal_event(handle.trigger_key))
        );

        let after_b = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyB,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(50),
        );
        assert!(
            after_b
                .iter()
                .any(|m| m.get_event() == signal_event(handle.trigger_key))
        );
    }

    #[test]
    fn sequence_binding_ignores_intervening_mouse_motion() {
        // Regression: mouse motion events used to be pushed into input_history and
        // caused the sequence tail check to fail. The corrected matching skips
        // unrelated events.
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Sequence {
                events: vec![
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyA)),
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyB)),
                ],
                within: Duration::from_millis(500),
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyA,
                pressed: true,
            }),
            None,
            now,
        );
        // Mouse motion in the middle of the sequence.
        for i in 0..10 {
            system.update_with_chaos_events(
                Some(ChaosInputEvent::MousePosition {
                    x: i as f64,
                    y: i as f64,
                }),
                None,
                now + Duration::from_millis(10 + i as u64),
            );
        }
        let after_b = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyB,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(100),
        );

        assert!(
            after_b
                .iter()
                .any(|m| m.get_event() == signal_event(handle.trigger_key))
        );
    }

    #[test]
    fn sequence_binding_tolerates_release_between_repeated_presses() {
        // Regression: [Pressed(A), Pressed(A)] used to be blocked by the intervening
        // Released(A) sitting in input_history between the two presses.
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Sequence {
                events: vec![
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyA)),
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyA)),
                ],
                within: Duration::from_millis(500),
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyA,
                pressed: true,
            }),
            None,
            now,
        );
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyA,
                pressed: false,
            }),
            None,
            now + Duration::from_millis(50),
        );
        let after_second_press = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyA,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(100),
        );

        assert!(
            after_second_press
                .iter()
                .any(|m| m.get_event() == signal_event(handle.trigger_key))
        );
    }

    #[test]
    fn sequence_binding_does_not_fire_when_within_window_expired() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Sequence {
                events: vec![
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyA)),
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyB)),
                ],
                within: Duration::from_millis(100),
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyA,
                pressed: true,
            }),
            None,
            now,
        );
        let after_b = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyB,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(500),
        );

        assert!(
            after_b
                .iter()
                .all(|m| m.get_event() != signal_event(handle.trigger_key))
        );
    }

    #[test]
    fn chord_binding_fires_once_when_completed_and_not_on_unrelated_events() {
        // Regression: Chord bindings used to re-fire on every subsequent input event
        // (e.g. mouse motion) while all chord keys remained pressed.
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Chord {
                keys: vec![
                    ChaosButton::Keyboard(KeyCode::ControlLeft),
                    ChaosButton::Keyboard(KeyCode::KeyS),
                ],
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        let after_ctrl = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::ControlLeft,
                pressed: true,
            }),
            None,
            now,
        );
        assert!(
            after_ctrl
                .iter()
                .all(|m| m.get_event() != signal_event(handle.trigger_key))
        );

        let after_s = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyS,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(10),
        );
        assert_eq!(
            after_s
                .iter()
                .filter(|m| m.get_event() == signal_event(handle.trigger_key))
                .count(),
            1
        );

        // Mouse motion while chord is held must not re-fire.
        let after_mouse = system.update_with_chaos_events(
            Some(ChaosInputEvent::MousePosition { x: 1.0, y: 2.0 }),
            None,
            now + Duration::from_millis(20),
        );
        assert!(
            after_mouse
                .iter()
                .all(|m| m.get_event() != signal_event(handle.trigger_key))
        );

        // Releasing and re-pressing a chord key should allow the chord to fire again.
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyS,
                pressed: false,
            }),
            None,
            now + Duration::from_millis(30),
        );
        let after_s_again = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyS,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(40),
        );
        assert_eq!(
            after_s_again
                .iter()
                .filter(|m| m.get_event() == signal_event(handle.trigger_key))
                .count(),
            1
        );
    }

    #[test]
    fn chord_binding_does_not_fire_on_release_of_chord_key() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Chord {
                keys: vec![
                    ChaosButton::Keyboard(KeyCode::ControlLeft),
                    ChaosButton::Keyboard(KeyCode::KeyS),
                ],
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::ControlLeft,
                pressed: true,
            }),
            None,
            now,
        );
        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyS,
                pressed: true,
            }),
            None,
            now + Duration::from_millis(10),
        );
        let after_release = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyS,
                pressed: false,
            }),
            None,
            now + Duration::from_millis(20),
        );

        assert!(
            after_release
                .iter()
                .all(|m| m.get_event() != signal_event(handle.trigger_key))
        );
    }

    #[test]
    fn unbind_removes_binding() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Input(ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(
                KeyCode::Space,
            ))),
            TestSignal::Fire,
        );

        assert!(system.unbind(handle.id));
        assert!(!system.unbind(handle.id));

        let messages = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::Space,
                pressed: true,
            }),
            None,
            Instant::now(),
        );

        assert!(messages.is_empty());
    }

    #[test]
    fn mouse_position_events_do_not_evict_sequence_history() {
        // Regression check for the input_history filtering: with only a keyboard
        // sequence registered, a flood of mouse motion events must not push older
        // key presses out of the history buffer.
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Sequence {
                events: vec![
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyA)),
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(KeyCode::KeyB)),
                ],
                within: Duration::from_secs(10),
            },
            TestSignal::Fire,
        );
        let now = Instant::now();

        system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyA,
                pressed: true,
            }),
            None,
            now,
        );
        for i in 0..(MAX_INPUT_HISTORY * 4) {
            system.update_with_chaos_events(
                Some(ChaosInputEvent::MousePosition {
                    x: i as f64,
                    y: i as f64,
                }),
                None,
                now + Duration::from_millis(1 + i as u64),
            );
        }
        let after_b = system.update_with_chaos_events(
            Some(ChaosInputEvent::KeyboardInput {
                keycode: KeyCode::KeyB,
                pressed: true,
            }),
            None,
            now + Duration::from_secs(1),
        );

        assert!(
            after_b
                .iter()
                .any(|m| m.get_event() == signal_event(handle.trigger_key))
        );
    }

    #[test]
    fn signal_message_carries_device_event_params() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Device(ChaosDeviceEventMatcher::Resized),
            TestSignal::Close,
        );

        let messages = system.update_with_chaos_events(
            None,
            Some(ChaosDeviceEvent::Resized(1280, 720)),
            Instant::now(),
        );

        let signal_message = messages
            .iter()
            .find(|m| m.get_event() == signal_event(handle.trigger_key))
            .expect("signal message");
        assert_eq!(
            signal_message.get::<TestSignal>("signal"),
            Some(TestSignal::Close)
        );
        assert_eq!(signal_message.get::<u32>("width"), Some(1280));
        assert_eq!(signal_message.get::<u32>("height"), Some(720));
        assert_eq!(
            signal_message.get::<ChaosDeviceEvent>("device_event"),
            Some(ChaosDeviceEvent::Resized(1280, 720))
        );
    }

    #[test]
    fn signal_message_carries_mouse_position_params() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
            ChaosBindingEvent::Input(ChaosInputEventMatcher::MouseMoved),
            TestSignal::Fire,
        );

        let messages = system.update_with_chaos_events(
            Some(ChaosInputEvent::MousePosition { x: 12.5, y: -3.25 }),
            None,
            Instant::now(),
        );

        let signal_message = messages
            .iter()
            .find(|m| m.get_event() == signal_event(handle.trigger_key))
            .expect("signal message");
        assert_eq!(
            signal_message.get::<TestSignal>("signal"),
            Some(TestSignal::Fire)
        );
        assert_eq!(signal_message.get::<f64>("x"), Some(12.5));
        assert_eq!(signal_message.get::<f64>("y"), Some(-3.25));
    }

    #[test]
    fn signal_message_carries_keyboard_input_params() {
        let mut system = DeviceEventSystem::new();
        let handle = system.bind(
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

        let signal_message = messages
            .iter()
            .find(|m| m.get_event() == signal_event(handle.trigger_key))
            .expect("signal message");
        assert_eq!(
            signal_message.get::<KeyCode>("keycode"),
            Some(KeyCode::Space)
        );
        assert_eq!(signal_message.get::<bool>("pressed"), Some(true));
    }
}
