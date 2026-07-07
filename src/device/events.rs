use winit::event::{MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use chaos_communicator::message::{ChaosMessage, ChaosMessageBuilder};

use crate::device::bindings::{ChaosButton, ChaosDeviceEventMatcher, ChaosInputEventMatcher};

pub type ChaosKeyCode = KeyCode;

pub type ChaosMouseButton = MouseButton;

#[derive(Clone, Debug, PartialEq)]
pub enum ChaosInputEvent {
    KeyboardInput {
        keycode: ChaosKeyCode,
        pressed: bool,
    },
    MousePosition {
        x: f64,
        y: f64,
    },
    MouseButton {
        button: ChaosMouseButton,
        pressed: bool,
    },
    MouseWheel {
        delta_x: f32,
        delta_y: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChaosDeviceEvent {
    CloseRequested,
    Focused,
    Unfocused,
    Resized(u32, u32),
    Moved(i32, i32),
    MouseEntered,
    MouseExited,
}

// converter for winit::Event to ChaosInputEvent, only converts the input events that we care about
// and returns an error for the rest
impl TryFrom<&WindowEvent> for ChaosInputEvent {
    type Error = ();
    fn try_from(event: &WindowEvent) -> Result<Self, Self::Error> {
        match event {
            WindowEvent::KeyboardInput { event, .. } => match event.physical_key {
                PhysicalKey::Unidentified(_) => Err(()),
                PhysicalKey::Code(key) => Ok(ChaosInputEvent::KeyboardInput {
                    keycode: key,
                    pressed: event.state == winit::event::ElementState::Pressed,
                }),
            },
            WindowEvent::MouseInput { button, state, .. } => match button {
                MouseButton::Left | MouseButton::Right | MouseButton::Middle => {
                    Ok(ChaosInputEvent::MouseButton {
                        button: button.clone(),
                        pressed: *state == winit::event::ElementState::Pressed,
                    })
                }
                _ => Err(()),
            },
            WindowEvent::CursorMoved { position, .. } => Ok(ChaosInputEvent::MousePosition {
                x: position.x,
                y: position.y,
            }),
            WindowEvent::MouseWheel { delta, .. } => match delta {
                winit::event::MouseScrollDelta::LineDelta(x, y) => {
                    Ok(ChaosInputEvent::MouseWheel {
                        delta_x: *x,
                        delta_y: *y,
                    })
                }
                winit::event::MouseScrollDelta::PixelDelta(pos) => {
                    Ok(ChaosInputEvent::MouseWheel {
                        delta_x: pos.x as f32,
                        delta_y: pos.y as f32,
                    })
                }
            },
            _ => Err(()),
        }
    }
}

impl TryFrom<&WindowEvent> for ChaosDeviceEvent {
    type Error = ();
    fn try_from(event: &WindowEvent) -> Result<Self, Self::Error> {
        match event {
            WindowEvent::CloseRequested => Ok(ChaosDeviceEvent::CloseRequested),
            WindowEvent::Focused(focused) => {
                if *focused {
                    Ok(ChaosDeviceEvent::Focused)
                } else {
                    Ok(ChaosDeviceEvent::Unfocused)
                }
            }
            WindowEvent::Resized(size) => Ok(ChaosDeviceEvent::Resized(size.width, size.height)),
            WindowEvent::Moved(position) => Ok(ChaosDeviceEvent::Moved(position.x, position.y)),
            WindowEvent::CursorEntered { .. } => Ok(ChaosDeviceEvent::MouseEntered),
            WindowEvent::CursorLeft { .. } => Ok(ChaosDeviceEvent::MouseExited),
            _ => Err(()),
        }
    }
}

impl ChaosInputEvent {
    /// Extend `builder` with the parameters relevant to this input event, so downstream
    /// consumers of a signal message can inspect e.g. cursor position or wheel delta
    /// without also having to unpack the `input_event` param.
    pub(crate) fn enrich_message(&self, builder: ChaosMessageBuilder) -> ChaosMessageBuilder {
        let builder = builder.with_param("input_event", self.clone());
        match self {
            ChaosInputEvent::KeyboardInput { keycode, pressed } => builder
                .with_param("keycode", *keycode)
                .with_param("pressed", *pressed),
            ChaosInputEvent::MousePosition { x, y } => {
                builder.with_param("x", *x).with_param("y", *y)
            }
            ChaosInputEvent::MouseButton { button, pressed } => builder
                .with_param("button", *button)
                .with_param("pressed", *pressed),
            ChaosInputEvent::MouseWheel { delta_x, delta_y } => builder
                .with_param("delta_x", *delta_x)
                .with_param("delta_y", *delta_y),
        }
    }
}

impl ChaosDeviceEvent {
    /// Extend `builder` with the parameters relevant to this device event (e.g.
    /// `width`/`height` for `Resized`).
    pub(crate) fn enrich_message(&self, builder: ChaosMessageBuilder) -> ChaosMessageBuilder {
        let builder = builder.with_param("device_event", self.clone());
        match self {
            ChaosDeviceEvent::Resized(width, height) => builder
                .with_param("width", *width)
                .with_param("height", *height),
            ChaosDeviceEvent::Moved(x, y) => builder.with_param("x", *x).with_param("y", *y),
            ChaosDeviceEvent::CloseRequested
            | ChaosDeviceEvent::Focused
            | ChaosDeviceEvent::Unfocused
            | ChaosDeviceEvent::MouseEntered
            | ChaosDeviceEvent::MouseExited => builder,
        }
    }
}

impl From<ChaosInputEvent> for ChaosMessage {
    fn from(event: ChaosInputEvent) -> Self {
        let builder = event.enrich_message(ChaosMessageBuilder::new());
        match &event {
            ChaosInputEvent::KeyboardInput { keycode, pressed } => {
                let event_key = if *pressed {
                    ChaosInputEventMatcher::Pressed(ChaosButton::Keyboard(*keycode))
                } else {
                    ChaosInputEventMatcher::Released(ChaosButton::Keyboard(*keycode))
                };
                builder.build_for_event(event_key)
            }
            ChaosInputEvent::MousePosition { .. } => {
                builder.build_for_event(ChaosInputEventMatcher::MouseMoved)
            }
            ChaosInputEvent::MouseButton { button, pressed } => {
                let event_key = if *pressed {
                    ChaosInputEventMatcher::Pressed(ChaosButton::Mouse(*button))
                } else {
                    ChaosInputEventMatcher::Released(ChaosButton::Mouse(*button))
                };
                builder.build_for_event(event_key)
            }
            ChaosInputEvent::MouseWheel { .. } => {
                builder.build_for_event(ChaosInputEventMatcher::MouseWheel)
            }
        }
    }
}

impl From<ChaosDeviceEvent> for ChaosMessage {
    fn from(event: ChaosDeviceEvent) -> Self {
        let builder = event.enrich_message(ChaosMessageBuilder::new());
        let key = match &event {
            ChaosDeviceEvent::CloseRequested => ChaosDeviceEventMatcher::CloseRequested,
            ChaosDeviceEvent::Focused => ChaosDeviceEventMatcher::Focused,
            ChaosDeviceEvent::Unfocused => ChaosDeviceEventMatcher::Unfocused,
            ChaosDeviceEvent::Resized(..) => ChaosDeviceEventMatcher::Resized,
            ChaosDeviceEvent::Moved(..) => ChaosDeviceEventMatcher::Moved,
            ChaosDeviceEvent::MouseEntered => ChaosDeviceEventMatcher::MouseEntered,
            ChaosDeviceEvent::MouseExited => ChaosDeviceEventMatcher::MouseExited,
        };
        builder.build_for_event(key)
    }
}
