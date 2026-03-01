use std::path::PathBuf;

use thiserror::Error;

/// Error surface for daemon runtime, protocol, and launchd management.
#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),

    #[error("registry error: {0}")]
    Registry(#[from] orchestra_core::RegistryError),

    #[error("sync error: {0}")]
    Sync(#[from] orchestra_sync::SyncError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("channel closed: {0}")]
    ChannelClosed(&'static str),

    #[error("daemon protocol error: {0}")]
    Protocol(String),

    #[error("daemon is not running (socket missing: {socket})")]
    DaemonNotRunning { socket: PathBuf },

    #[error("launchd error: {0}")]
    Launchd(String),
}

pub(crate) fn io_err(path: impl Into<PathBuf>, source: std::io::Error) -> DaemonError {
    DaemonError::Io {
        path: path.into(),
        source,
    }
}
