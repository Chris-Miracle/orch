//! Error types for orchestra-renderer.

use std::path::PathBuf;

use thiserror::Error;

/// All errors that can arise from template rendering operations.
#[derive(Debug, Error)]
pub enum RenderError {
    /// Tera template engine error.
    #[error("template engine error: {0}")]
    Tera(#[from] tera::Error),

    /// JSON serialization error (building tera context).
    #[error("context serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Filesystem error while loading user templates.
    #[error("template io error at {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
}
