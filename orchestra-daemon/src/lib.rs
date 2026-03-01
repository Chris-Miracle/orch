//! Phase 04 daemon runtime: watcher + sync processor + socket server.

mod error;
pub mod launchd;
pub mod log_rotation;
pub mod paths;
pub mod protocol;
mod runtime;

pub use error::DaemonError;
pub use launchd::{generate_plist, install as install_launchd, uninstall as uninstall_launchd};
pub use protocol::{
    request_status, request_stop, request_sync, send_request, DaemonRequest, DaemonResponse,
};
pub use runtime::{run, start_blocking, RegistryCache, SyncSummary};
