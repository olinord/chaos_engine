//! Errors for the rendering layer.
//!
//! Policy (see the frame loop in `engine.rs` for the one place that acts on these):
//!
//! - **Bugs** (bad arguments to a command, recording outside a rendering scope, ...) are
//!   *not* represented here. They panic, because no caller can recover from them.
//!   Command recording in vulkano can only fail with a `ValidationError`, so the
//!   recording methods on `ChaosCommandBuffer` panic instead of returning a `Result`.
//! - **Expected runtime conditions** (`SwapchainOutOfDate`) are handled inside the
//!   renderer by recreating the swapchain and skipping the frame.
//! - **Everything else** that escapes the renderer is treated as fatal by the engine.
//!
//! Variants are organised by *cause*, not by which function failed, so callers can
//! `match` on them and react differently. The failing operation is kept as a
//! `&'static str` for the message, and the original error is kept as the `source`.

use std::error::Error;
use std::fmt;

use vulkano::{Validated, ValidationError, VulkanError};

use crate::rendering::graph::graph::{GraphBuildError, GraphError};

#[derive(Debug)]
pub enum RenderError {
    /// The swapchain no longer matches the surface (e.g. after a resize).
    /// The renderer recreates the swapchain and skips the frame.
    SwapchainOutOfDate,

    /// The GPU device was lost (driver crash/reset, GPU removed). Fatal.
    DeviceLost { op: &'static str },

    /// Host or device memory ran out while doing `op`.
    OutOfMemory { op: &'static str },

    /// The renderer couldn't be set up (no Vulkan driver, no suitable GPU, ...).
    Initialization {
        what: &'static str,
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    /// Allocating a GPU resource (buffer, image) failed.
    Allocation {
        name: String,
        source: Box<dyn Error + Send + Sync>,
    },

    /// A buffer was used before any data was uploaded to it.
    BufferNotUploaded { name: String },

    /// vulkano rejected the parameters of a non-recording call, e.g. descriptor set
    /// writes that don't match a (possibly hot-reloaded) shader's layout.
    Validation {
        op: &'static str,
        source: Box<ValidationError>,
    },
    GraphError {
        op: &'static str,
        source: Box<GraphError>,
    },
    GraphBuildError {
        op: &'static str,
        source: Box<GraphBuildError>,
    },
    /// Any other Vulkan failure.
    Vulkan {
        op: &'static str,
        source: VulkanError,
    },
    Custom {
        op: &'static str,
        message: String,
        recoverable: bool,
    },
}

impl RenderError {
    /// Classifies a raw `VulkanError` into the variant a caller can act on.
    pub fn from_vulkan(op: &'static str, error: VulkanError) -> Self {
        match error {
            VulkanError::OutOfDate => RenderError::SwapchainOutOfDate,
            VulkanError::DeviceLost => RenderError::DeviceLost { op },
            VulkanError::OutOfHostMemory | VulkanError::OutOfDeviceMemory => {
                RenderError::OutOfMemory { op }
            }
            source => RenderError::Vulkan { op, source },
        }
    }

    pub fn from_validated(op: &'static str, error: Validated<VulkanError>) -> Self {
        match error {
            Validated::Error(error) => Self::from_vulkan(op, error),
            Validated::ValidationError(source) => RenderError::Validation { op, source },
        }
    }

    pub fn initialization(what: &'static str, source: impl Error + Send + Sync + 'static) -> Self {
        RenderError::Initialization {
            what,
            source: Some(Box::new(source)),
        }
    }

    pub fn allocation(name: impl Into<String>, source: impl Error + Send + Sync + 'static) -> Self {
        RenderError::Allocation {
            name: name.into(),
            source: Box::new(source),
        }
    }

    pub fn recoverable(&self) -> bool {
        match *self {
            RenderError::DeviceLost { .. }
            | RenderError::OutOfMemory { .. }
            | RenderError::Initialization { .. }
            | RenderError::Allocation { .. }
            | RenderError::GraphError { .. }
            | RenderError::GraphBuildError { .. }
            | RenderError::Vulkan { .. }
            | RenderError::Validation { .. } => false,
            RenderError::SwapchainOutOfDate | RenderError::BufferNotUploaded { .. } => true,
            RenderError::Custom { recoverable, .. } => recoverable,
        }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use RenderError::*;
        match self {
            SwapchainOutOfDate => write!(f, "swapchain is out of date"),
            DeviceLost { op } => write!(f, "GPU device lost during {op}"),
            OutOfMemory { op } => write!(f, "out of memory during {op}"),
            Initialization { what, .. } => write!(f, "failed to initialize renderer: {what}"),
            Allocation { name, .. } => write!(f, "failed to allocate '{name}'"),
            BufferNotUploaded { name } => write!(f, "buffer '{name}' has no data uploaded"),
            Validation { op, .. } => write!(f, "invalid parameters for {op}"),
            Vulkan { op, .. } => write!(f, "Vulkan call failed during {op}"),
            GraphError { op, .. } => write!(f, "Graph call failed during {op}"),
            GraphBuildError { op, .. } => write!(f, "Graph build call failed during {op}"),
            Custom { op, .. } => write!(f, "Error during {op}"),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use RenderError::*;

        match self {
            Initialization { source, .. } => source.as_deref().map(|e| e as &(dyn Error + 'static)),
            Allocation { source, .. } => Some(source.as_ref()),
            Validation { source, .. } => Some(source.as_ref()),
            Vulkan { source, .. } => Some(source),
            GraphError { source, .. } => Some(source),
            GraphBuildError { source, .. } => Some(source),
            SwapchainOutOfDate
            | DeviceLost { .. }
            | OutOfMemory { .. }
            | BufferNotUploaded { .. }
            | Custom { .. } => None,
        }
    }
}

/// Attaches the name of the failing operation to a vulkano result, so call sites read
/// `RecordingCommandBuffer::new(..).context("begin command buffer")?`.
pub trait RenderResultExt<T> {
    fn context(self, op: &'static str) -> Result<T, RenderError>;
}

impl<T> RenderResultExt<T> for Result<T, VulkanError> {
    fn context(self, op: &'static str) -> Result<T, RenderError> {
        self.map_err(|e| RenderError::from_vulkan(op, e))
    }
}

impl<T> RenderResultExt<T> for Result<T, Validated<VulkanError>> {
    fn context(self, op: &'static str) -> Result<T, RenderError> {
        self.map_err(|e| RenderError::from_validated(op, e))
    }
}

impl<T> RenderResultExt<T> for Result<T, GraphError> {
    fn context(self, op: &'static str) -> Result<T, RenderError> {
        self.map_err(|e| RenderError::GraphError {
            op,
            source: Box::new(e),
        })
    }
}

impl<T> RenderResultExt<T> for Result<T, GraphBuildError> {
    fn context(self, op: &'static str) -> Result<T, RenderError> {
        self.map_err(|e| RenderError::GraphBuildError {
            op,
            source: Box::new(e),
        })
    }
}

impl<T> RenderResultExt<T> for Result<T, &'static str> {
    fn context(self, op: &'static str) -> Result<T, RenderError> {
        self.map_err(|e| RenderError::Custom {
            op,
            message: e.to_string(),
            recoverable: false,
        })
    }
}

impl<T> RenderResultExt<T> for Result<T, String> {
    fn context(self, op: &'static str) -> Result<T, RenderError> {
        self.map_err(|e| RenderError::Custom {
            op,
            message: e,
            recoverable: false,
        })
    }
}

/// Unwraps the result of recording a command. vulkano only fails command recording
/// with a `ValidationError`, which means the engine passed bad arguments: a bug, so panic
/// with a message that names the command.
#[track_caller]
pub(crate) fn expect_valid<T>(result: Result<T, Box<ValidationError>>, command: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("invalid `{command}` command: {error}"),
    }
}

/// Formats an error with its whole `source()` chain on one line:
/// `failed to allocate 'x': out of device memory: ...`.
pub fn error_chain(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut current = error.source();
    while let Some(source) = current {
        message.push_str(": ");
        message.push_str(&source.to_string());
        current = source.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_date_is_classified() {
        assert!(matches!(
            RenderError::from_vulkan("present", VulkanError::OutOfDate),
            RenderError::SwapchainOutOfDate
        ));
    }

    #[test]
    fn memory_errors_are_classified() {
        assert!(matches!(
            RenderError::from_vulkan("submit", VulkanError::OutOfDeviceMemory),
            RenderError::OutOfMemory { op: "submit" }
        ));
        assert!(matches!(
            RenderError::from_vulkan("submit", VulkanError::OutOfHostMemory),
            RenderError::OutOfMemory { op: "submit" }
        ));
    }

    #[test]
    fn other_errors_keep_their_source() {
        let error = RenderError::from_vulkan("submit", VulkanError::Unknown);
        assert!(matches!(error, RenderError::Vulkan { op: "submit", .. }));
        assert!(error.source().is_some());
    }

    #[test]
    fn error_chain_includes_sources() {
        let error = RenderError::from_vulkan("submit", VulkanError::Unknown);
        let message = error_chain(&error);
        assert!(message.starts_with("Vulkan call failed during submit: "));
    }
}
