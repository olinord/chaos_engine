use winit::{Event, WindowEvent, DeviceEvent, ElementState, VirtualKeyCode};
use std::time::Instant;

pub type KeyCode = VirtualKeyCode;

// Event to use in registration
#[derive(Hash, Eq, Copy, Clone)]
pub enum ChaosDeviceEventRegistration {
    CloseRequested,
    Focused,
    Unfocused,
    Suspended,
    Reanimated,
    Resized,
    // MouseEntered,
    // MouseExited,
    KeyPress(KeyCode), // only handles keyboard input for now (keycode and if it is pressed)
    MultiKeyPress(KeyCode, u128)
    // InputMovement(InputMovementEvent),
}

// Event that is created from a winit event
#[derive(Hash, Eq, Debug)]
pub enum ChaosDeviceEvent {
    CloseRequested,
    Focused,
    Unfocused,
    Suspended,
    Reanimated,
    // Resized(f64, f64),
    KeyPress(KeyCode, bool), // only handles keyboard input for now (keycode and if it is pressed)
    Unrecognized
}

// key state storage (to store how often we have pressed etc)
#[derive(Hash, Eq)]
pub struct ChaosDeviceEventState {
    button: KeyCode,
    repeats: u128,
    pressed: bool,
    last_release_time: Instant,
    last_pressed_time: Instant
}

impl std::cmp::PartialEq<ChaosDeviceEvent> for ChaosDeviceEvent {
    fn eq(&self, other: &ChaosDeviceEvent) -> bool {
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return false;
        }
        match (self, other) {
            (ChaosDeviceEvent::KeyPress(self_key, self_pressed), ChaosDeviceEvent::KeyPress(other_key, other_pressed)) => {
                self_key == other_key && self_pressed == other_pressed
            },
            (_, _) => true
        }
    }

    fn ne(&self, other: &ChaosDeviceEvent) -> bool {
        !self.eq(other)
    }
}

// Converter for winit::DeviceEvent to ChaosDeviceEvent
impl From<&winit::Event> for ChaosDeviceEvent {
    fn from(event: &Event) -> Self {
        match event {
            Event::Suspended(ev) => {
                if *ev {
                    ChaosDeviceEvent::Suspended
                }
                else {
                    ChaosDeviceEvent::Reanimated
                }
            },
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => ChaosDeviceEvent::CloseRequested,
                    WindowEvent::Focused(b) => {
                        if *b {
                            ChaosDeviceEvent::Focused
                        }
                        else {
                            ChaosDeviceEvent::Unfocused
                        }
                    },
                    WindowEvent::KeyboardInput {input, ..} => {
                        if let Some(virtual_keycode) = input.virtual_keycode {
                            return match input.state {
                                ElementState::Pressed => ChaosDeviceEvent::KeyPress(virtual_keycode, true),
                                ElementState::Released => ChaosDeviceEvent::KeyPress(virtual_keycode, false),
                            }
                        }
                        ChaosDeviceEvent::Unrecognized
                    },
                    // WindowEvent::Resized(ls) => ChaosDeviceEvent::Resized(ls.width, ls.height),
                    // WindowEvent::CursorEntered { .. } => ChaosDeviceEvent::MouseEntered,
                    // WindowEvent::CursorLeft { .. } => ChaosDeviceEvent::MouseExited,
                    _ => ChaosDeviceEvent::Unrecognized,
                }
            },
            Event::DeviceEvent { event, .. } => {
                match event {
                    DeviceEvent::Key(input) => {
                        if let Some(virtual_keycode) = input.virtual_keycode {
                            match input.state {
                                ElementState::Pressed => ChaosDeviceEvent::KeyPress(virtual_keycode, true),
                                ElementState::Released => ChaosDeviceEvent::KeyPress(virtual_keycode, false),
                            };
                        }
                        ChaosDeviceEvent::Unrecognized
                    }
                    _ => ChaosDeviceEvent::Unrecognized
                }
            }
            _ => ChaosDeviceEvent::Unrecognized
        }
    }
}

impl From<ChaosDeviceEventRegistration> for ChaosDeviceEvent {
    fn from(cder: ChaosDeviceEventRegistration) -> Self {
        match cder {
            ChaosDeviceEventRegistration::CloseRequested => ChaosDeviceEvent::CloseRequested,
            ChaosDeviceEventRegistration::KeyPress(key) => ChaosDeviceEvent::KeyPress(key, true),
            ChaosDeviceEventRegistration::MultiKeyPress(key, _) => ChaosDeviceEvent::KeyPress(key, true),
            ChaosDeviceEventRegistration::Suspended => ChaosDeviceEvent::Suspended,
            ChaosDeviceEventRegistration::Reanimated => ChaosDeviceEvent::Reanimated,
            ChaosDeviceEventRegistration::Focused => ChaosDeviceEvent::Focused,
            ChaosDeviceEventRegistration::Unfocused => ChaosDeviceEvent::Unfocused,
            // ChaosDeviceEventRegistration::Resized => ChaosDeviceEvent::Resized,
            _ => ChaosDeviceEvent::Unrecognized
        }
    }
}

impl std::cmp::PartialEq<ChaosDeviceEventRegistration> for ChaosDeviceEventRegistration {
    fn eq(&self, other: &ChaosDeviceEventRegistration) -> bool {
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return false;
        }

        match (self, other) {
            (ChaosDeviceEventRegistration::KeyPress(self_key_press), ChaosDeviceEventRegistration::KeyPress(other_key_press)) => {
                self_key_press == other_key_press
            },
            (ChaosDeviceEventRegistration::MultiKeyPress(self_key_press, self_repeats), ChaosDeviceEventRegistration::MultiKeyPress(other_key_press, other_repeats)) => {
                self_key_press == other_key_press && self_repeats == other_repeats
            },
            (_, _) => true
        }
    }

    fn ne(&self, other: &ChaosDeviceEventRegistration) -> bool {
        return !self.eq(other);
    }
}

impl std::cmp::PartialEq<ChaosDeviceEvent> for ChaosDeviceEventRegistration {
    fn eq(&self, other: &ChaosDeviceEvent) -> bool {
        // Only makes sure that the ChaosDeviceEvent is mapped to the same key as the ChaoInputEventRegistration
        match other {
            ChaosDeviceEvent::KeyPress(input_key, event_pressed) => {
                match self {
                    ChaosDeviceEventRegistration::KeyPress(registration_input_key) => {
                        // just when the button is pressed do we get a hit
                        *event_pressed && input_key == registration_input_key
                    },
                    _ => false
                }
            },
            ChaosDeviceEvent::Unfocused => *self == ChaosDeviceEventRegistration::Unfocused,
            ChaosDeviceEvent::Reanimated => *self == ChaosDeviceEventRegistration::Reanimated,
            // ChaosDeviceEvent::Resized(..) => self == ChaosDeviceEventRegistration::Resized,
            ChaosDeviceEvent::Focused => *self == ChaosDeviceEventRegistration::Focused,
            ChaosDeviceEvent::Suspended => *self == ChaosDeviceEventRegistration::Suspended,
            ChaosDeviceEvent::CloseRequested => *self == ChaosDeviceEventRegistration::CloseRequested,
            _ => false
        }
    }

    fn ne(&self, other: &ChaosDeviceEvent) -> bool {
        return !self.eq(other)
    }
}

impl ChaosDeviceEventState {
    pub fn new_single(key: KeyCode) -> ChaosDeviceEventState {
        return ChaosDeviceEventState {
            pressed: false,
            repeats: 0,
            last_pressed_time: Instant::now(),
            last_release_time: Instant::now(),
            button: key
        }
    }

    pub fn new_multi(key: KeyCode, repeats: u128) -> ChaosDeviceEventState {
        return ChaosDeviceEventState {
            pressed: false,
            repeats,
            last_pressed_time: Instant::now(),
            last_release_time: Instant::now(),
            button: key
        }
    }

    pub fn update(&mut self, device_event: &ChaosDeviceEvent, multi_press_time_delta_millis: u128) {
        match device_event {
            ChaosDeviceEvent::KeyPress(event_key_code, event_pressed) => {
                let pressed = *event_pressed;
                if *event_key_code == self.button && self.pressed != pressed
                {
                    let now = Instant::now();
                    // check if we are multi pressing
                    if pressed && now.duration_since(self.last_pressed_time).as_millis() < multi_press_time_delta_millis {
                        self.repeats += 1;
                    }
                    if self.pressed {
                        self.last_release_time = now;
                    }
                    else {
                        self.last_pressed_time = now;
                    }
                    self.pressed = !self.pressed;
                }
            },
            _ => return
        };

    }
}


impl std::cmp::PartialEq<ChaosDeviceEventState> for ChaosDeviceEventState {
    fn eq(&self, other: &ChaosDeviceEventState) -> bool {
        return self.button == other.button;
    }

    fn ne(&self, other: &ChaosDeviceEventState) -> bool {
        !self.eq(other)
    }
}

impl std::cmp::PartialEq<ChaosDeviceEventRegistration> for ChaosDeviceEventState {
    fn eq(&self, other: &ChaosDeviceEventRegistration) -> bool {
        match other {
            ChaosDeviceEventRegistration::KeyPress(key_code) => {
                self.button == *key_code && self.pressed
            },
            ChaosDeviceEventRegistration::MultiKeyPress(key_code, repeats) => {
                self.button == *key_code && self.pressed && self.repeats % repeats == 0
            },
            _ => false
        }
    }

    fn ne(&self, other: &ChaosDeviceEventRegistration) -> bool {
        return !self.eq(other);
    }
}