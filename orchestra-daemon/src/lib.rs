//! Phase 04 daemon runtime: watcher + sync processor + socket server.

mod error;
pub mod launchd;
pub mod log_rotation;
pub mod paths;
pub mod protocol;
#[cfg(unix)]
mod runtime;

pub use error::DaemonError;
pub use launchd::{generate_plist, install as install_launchd, uninstall as uninstall_launchd};
pub use protocol::{
    request_status, request_stop, request_sync, send_request, DaemonRequest, DaemonResponse,
};

#[cfg(unix)]
pub use runtime::{run, start_blocking, RegistryCache, SyncSummary};

// ---------------------------------------------------------------------------
// Windows stubs
// ---------------------------------------------------------------------------

#[cfg(not(unix))]
pub type RegistryCache =
    std::collections::HashMap<orchestra_core::types::CodebaseName, orchestra_core::types::Codebase>;

#[cfg(not(unix))]
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncSummary {
    pub target: String,
    pub source: String,
    pub codebases: Vec<String>,
    pub written: usize,
    pub unchanged: usize,
    pub duration_ms: u128,
}

#[cfg(not(unix))]
pub fn start_blocking(_home: &std::path::Path) -> Result<(), DaemonError> {
    Err(DaemonError::Protocol(
        "the Orchestra daemon is not supported on Windows".to_string(),
    ))
}

#[cfg(not(unix))]
pub async fn run(_home: std::path::PathBuf) -> Result<(), DaemonError> {
    Err(DaemonError::Protocol(
        "the Orchestra daemon is not supported on Windows".to_string(),
    ))
}

