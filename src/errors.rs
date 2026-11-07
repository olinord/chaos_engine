//! Top-level engine error. Each subsystem has its own error type; this wraps them so
//! `?` composes upward, and `ChaosEngine::run` hands the fatal one back to `main`.

use std::error::Error;
use std::fmt;

use winit::error::{EventLoopError, OsError};
use winit::raw_window_handle::HandleError;

use crate::rendering::errors::RenderError;

#[derive(Debug)]
pub enum EngineError {
    /// The OS event loop couldn't be created or failed while running.
    EventLoop(EventLoopError),
    /// The window couldn't be created.
    CreateWindow(OsError),
    /// The window's display handle couldn't be retrieved.
    DisplayHandle(HandleError),
    /// A fatal rendering error (device lost, out of memory, no suitable GPU, ...).
    Render(RenderError),
    /// A world system failed. Systems report errors as static strings.
    World(&'static str),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::EventLoop(_) => write!(f, "event loop error"),
            EngineError::CreateWindow(_) => write!(f, "failed to create window"),
            EngineError::DisplayHandle(_) => write!(f, "failed to get display handle"),
            EngineError::Render(_) => write!(f, "rendering failed"),
            EngineError::World(message) => write!(f, "world system failed: {message}"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EngineError::EventLoop(e) => Some(e),
            EngineError::CreateWindow(e) => Some(e),
            EngineError::DisplayHandle(e) => Some(e),
            EngineError::Render(e) => Some(e),
            EngineError::World(_) => None,
        }
    }
}

impl From<EventLoopError> for EngineError {
    fn from(error: EventLoopError) -> Self {
        EngineError::EventLoop(error)
    }
}

impl From<OsError> for EngineError {
    fn from(error: OsError) -> Self {
        EngineError::CreateWindow(error)
    }
}

impl From<HandleError> for EngineError {
    fn from(error: HandleError) -> Self {
        EngineError::DisplayHandle(error)
    }
}

impl From<RenderError> for EngineError {
    fn from(error: RenderError) -> Self {
        EngineError::Render(error)
    }
}
