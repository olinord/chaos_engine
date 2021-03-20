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

// This struct is to store the just what happened, just the basics and no details
#[derive(Hash, Debug, Eq)]
pub enum ChaosDeviceEvent {
    CloseRequested,
    Focused,
    Unfocused,
    Suspended,
    Reanimated,
    // Resized,
    KeyPress(KeyCode),
    Unrecognized
}

// Event that is created from a winit event
#[derive(Hash, Eq, Debug)]
pub enum ChaosDeviceDetailedEvent {
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
    last_pressed_time: Instant,
    actual_repeats: u128
}

impl From<ChaosDeviceDetailedEvent> for ChaosDeviceEvent {
    fn from(detailed: ChaosDeviceDetailedEvent) -> Self {
        match detailed {
            ChaosDeviceDetailedEvent::KeyPress(code, __) => ChaosDeviceEvent::KeyPress(code),
            ChaosDeviceDetailedEvent::Focused => ChaosDeviceEvent::Focused,
            ChaosDeviceDetailedEvent::CloseRequested => ChaosDeviceEvent::CloseRequested,
            ChaosDeviceDetailedEvent::Unfocused => ChaosDeviceEvent::Unfocused,
            ChaosDeviceDetailedEvent::Reanimated => ChaosDeviceEvent::Reanimated,
            ChaosDeviceDetailedEvent::Suspended => ChaosDeviceEvent::Suspended,
            ChaosDeviceDetailedEvent::Unrecognized => ChaosDeviceEvent::Unrecognized,
        }
    }
}

impl From<&ChaosDeviceDetailedEvent> for ChaosDeviceEvent {
    fn from(detailed: &ChaosDeviceDetailedEvent) -> Self {
        match detailed {
            ChaosDeviceDetailedEvent::KeyPress(code, __) => ChaosDeviceEvent::KeyPress(*code),
            ChaosDeviceDetailedEvent::Focused => ChaosDeviceEvent::Focused,
            ChaosDeviceDetailedEvent::CloseRequested => ChaosDeviceEvent::CloseRequested,
            ChaosDeviceDetailedEvent::Unfocused => ChaosDeviceEvent::Unfocused,
            ChaosDeviceDetailedEvent::Reanimated => ChaosDeviceEvent::Reanimated,
            ChaosDeviceDetailedEvent::Suspended => ChaosDeviceEvent::Suspended,
            ChaosDeviceDetailedEvent::Unrecognized => ChaosDeviceEvent::Unrecognized,
        }
    }
}

impl PartialEq for ChaosDeviceEvent {
    fn eq(&self, other: &ChaosDeviceEvent) -> bool {
        return std::mem::discriminant(self) == std::mem::discriminant(other);
    }

    fn ne(&self, other: &ChaosDeviceEvent) -> bool {
        !self.eq(other)
    }
}

impl PartialEq for ChaosDeviceDetailedEvent {
    fn eq(&self, other: &ChaosDeviceDetailedEvent) -> bool {
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return false;
        }
        match (self, other) {
            (ChaosDeviceDetailedEvent::KeyPress(self_key, self_pressed), ChaosDeviceDetailedEvent::KeyPress(other_key, other_pressed)) => {
                self_key == other_key && self_pressed == other_pressed
            },
            (_, _) => true
        }
    }

    fn ne(&self, other: &ChaosDeviceDetailedEvent) -> bool {
        !self.eq(other)
    }
}

// Converter for winit::Event to ChaosDeviceEvent
impl From<&winit::Event> for ChaosDeviceDetailedEvent {
    fn from(event: &Event) -> Self {
        match event {
            Event::Suspended(ev) => {
                if *ev {
                    ChaosDeviceDetailedEvent::Suspended
                }
                else {
                    ChaosDeviceDetailedEvent::Reanimated
                }
            },
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => ChaosDeviceDetailedEvent::CloseRequested,
                    WindowEvent::Focused(b) => {
                        if *b {
                            ChaosDeviceDetailedEvent::Focused
                        }
                        else {
                            ChaosDeviceDetailedEvent::Unfocused
                        }
                    },
                    WindowEvent::KeyboardInput {input, ..} => {
                        if let Some(virtual_keycode) = input.virtual_keycode {
                            return match input.state {
                                ElementState::Pressed => ChaosDeviceDetailedEvent::KeyPress(virtual_keycode, true),
                                ElementState::Released => ChaosDeviceDetailedEvent::KeyPress(virtual_keycode, false),
                            }
                        }
                        ChaosDeviceDetailedEvent::Unrecognized
                    },
                    // WindowEvent::Resized(ls) => ChaosDeviceEvent::Resized(ls.width, ls.height),
                    // WindowEvent::CursorEntered { .. } => ChaosDeviceEvent::MouseEntered,
                    // WindowEvent::CursorLeft { .. } => ChaosDeviceEvent::MouseExited,
                    _ => ChaosDeviceDetailedEvent::Unrecognized,
                }
            },
            Event::DeviceEvent { event, .. } => {
                match event {
                    DeviceEvent::Key(input) => {
                        if let Some(virtual_keycode) = input.virtual_keycode {
                            match input.state {
                                ElementState::Pressed => ChaosDeviceDetailedEvent::KeyPress(virtual_keycode, true),
                                ElementState::Released => ChaosDeviceDetailedEvent::KeyPress(virtual_keycode, false),
                            };
                        }
                        ChaosDeviceDetailedEvent::Unrecognized
                    }
                    _ => ChaosDeviceDetailedEvent::Unrecognized
                }
            }
            _ => ChaosDeviceDetailedEvent::Unrecognized
        }
    }
}

impl From<ChaosDeviceEventRegistration> for ChaosDeviceDetailedEvent {
    fn from(cder: ChaosDeviceEventRegistration) -> Self {
        match cder {
            ChaosDeviceEventRegistration::CloseRequested => ChaosDeviceDetailedEvent::CloseRequested,
            ChaosDeviceEventRegistration::KeyPress(key) => ChaosDeviceDetailedEvent::KeyPress(key, true),
            ChaosDeviceEventRegistration::MultiKeyPress(key, _) => ChaosDeviceDetailedEvent::KeyPress(key, true),
            ChaosDeviceEventRegistration::Suspended => ChaosDeviceDetailedEvent::Suspended,
            ChaosDeviceEventRegistration::Reanimated => ChaosDeviceDetailedEvent::Reanimated,
            ChaosDeviceEventRegistration::Focused => ChaosDeviceDetailedEvent::Focused,
            ChaosDeviceEventRegistration::Unfocused => ChaosDeviceDetailedEvent::Unfocused,
            // ChaosDeviceEventRegistration::Resized => ChaosDeviceEvent::Resized,
            _ => ChaosDeviceDetailedEvent::Unrecognized
        }
    }
}

impl PartialEq for ChaosDeviceEventRegistration {
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

impl PartialEq<ChaosDeviceDetailedEvent> for ChaosDeviceEventRegistration {
    fn eq(&self, other: &ChaosDeviceDetailedEvent) -> bool {
        // Only makes sure that the ChaosDeviceEvent is mapped to the same key as the ChaoInputEventRegistration
        match other {
            ChaosDeviceDetailedEvent::KeyPress(input_key, _) => {
                match self {
                    ChaosDeviceEventRegistration::KeyPress(registration_input_key) => {
                        input_key == registration_input_key
                    },
                    _ => false
                }
            },
            ChaosDeviceDetailedEvent::Unfocused => *self == ChaosDeviceEventRegistration::Unfocused,
            ChaosDeviceDetailedEvent::Reanimated => *self == ChaosDeviceEventRegistration::Reanimated,
            // ChaosDeviceEvent::Resized(..) => self == ChaosDeviceEventRegistration::Resized,
            ChaosDeviceDetailedEvent::Focused => *self == ChaosDeviceEventRegistration::Focused,
            ChaosDeviceDetailedEvent::Suspended => *self == ChaosDeviceEventRegistration::Suspended,
            ChaosDeviceDetailedEvent::CloseRequested => *self == ChaosDeviceEventRegistration::CloseRequested,
            _ => false
        }
    }

    fn ne(&self, other: &ChaosDeviceDetailedEvent) -> bool {
        return !self.eq(other)
    }
}

impl ChaosDeviceEventState {
    pub fn new_single(key: KeyCode) -> ChaosDeviceEventState {
        return ChaosDeviceEventState {
            pressed: false,
            repeats: 1,
            last_pressed_time: Instant::now(),
            last_release_time: Instant::now(),
            button: key,
            actual_repeats: 0
        }
    }

    pub fn new_multi(key: KeyCode, repeats: u128) -> ChaosDeviceEventState {
        return ChaosDeviceEventState {
            pressed: false,
            repeats,
            last_pressed_time: Instant::now(),
            last_release_time: Instant::now(),
            button: key,
            actual_repeats: 0
        }
    }

    pub fn update(&mut self, device_event: &ChaosDeviceDetailedEvent, multi_press_time_delta_millis: u128) {
        match device_event {
            ChaosDeviceDetailedEvent::KeyPress(event_key_code, event_pressed) => {
                if *event_key_code == self.button
                {
                    let now = Instant::now();
                    if *event_pressed {
                        // check if we are multi pressing
                        if now.duration_since(self.last_pressed_time).as_millis() < multi_press_time_delta_millis {
                            self.actual_repeats += 1;
                        } else {
                            // reset the repeats if we timed out
                            self.actual_repeats = 1;
                        }
                        self.last_release_time = now;
                    }
                    else{
                        self.last_pressed_time = now;
                    }

                    self.pressed = !self.pressed;
                }
            },
            _ => return
        };

    }
}


impl PartialEq for ChaosDeviceEventState {
    fn eq(&self, other: &ChaosDeviceEventState) -> bool {
        return self.button == other.button;
    }

    fn ne(&self, other: &ChaosDeviceEventState) -> bool {
        !self.eq(other)
    }
}

impl PartialEq<ChaosDeviceEventRegistration> for ChaosDeviceEventState {
    fn eq(&self, other: &ChaosDeviceEventRegistration) -> bool {
        match other {
            ChaosDeviceEventRegistration::KeyPress(key_code) => {
                self.button == *key_code && self.pressed
            },
            ChaosDeviceEventRegistration::MultiKeyPress(key_code, repeats) => {
                self.button == *key_code && self.pressed && self.actual_repeats % repeats == 0
            },
            _ => false
        }
    }

    fn ne(&self, other: &ChaosDeviceEventRegistration) -> bool {
        return !self.eq(other);
    }
}