//! Error types for orchestra-sync.

use std::path::PathBuf;

use thiserror::Error;

use orchestra_core::error::RegistryError;
use orchestra_renderer::RenderError;

/// All errors that can arise from sync operations.
#[derive(Debug, Error)]
pub enum SyncError {
    /// An error from the rendering engine.
    #[error("render error: {0}")]
    Render(#[from] RenderError),

    /// An error from the registry.
    #[error("registry error: {0}")]
    Registry(#[from] RegistryError),

    /// An I/O error, with annotated path for context.
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// JSON serialization/deserialization error (hash store).
    #[error("hash store JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience constructor for [`SyncError::Io`].
pub(crate) fn io_err(path: impl Into<PathBuf>, source: std::io::Error) -> SyncError {
    SyncError::Io {
        path: path.into(),
        source,
    }
}
