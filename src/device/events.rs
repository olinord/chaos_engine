use winit::event::{MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

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
    MouseEntered,
    MouseExited,
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
            WindowEvent::CursorEntered { .. } => Ok(ChaosInputEvent::MouseEntered),
            WindowEvent::CursorLeft { .. } => Ok(ChaosInputEvent::MouseExited),
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
