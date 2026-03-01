use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DAEMON_LABEL: &str = "dev.orchestra.daemon";
pub const DEBOUNCE_WINDOW: Duration = Duration::from_millis(500);

pub const DAEMON_STDOUT_LOG: &str = "daemon.log";
pub const DAEMON_STDERR_LOG: &str = "daemon-err.log";
pub const DAEMON_SOCKET: &str = "daemon.sock";

pub fn orchestra_root(home: &Path) -> PathBuf {
    home.join(".orchestra")
}

pub fn projects_root(home: &Path) -> PathBuf {
    orchestra_root(home).join("projects")
}

pub fn run_dir(home: &Path) -> PathBuf {
    orchestra_root(home).join("run")
}

pub fn socket_path(home: &Path) -> PathBuf {
    orchestra_root(home).join(DAEMON_SOCKET)
}

pub fn logs_dir(home: &Path) -> PathBuf {
    orchestra_root(home).join("logs")
}

pub fn stdout_log_path(home: &Path) -> PathBuf {
    logs_dir(home).join(DAEMON_STDOUT_LOG)
}

pub fn stderr_log_path(home: &Path) -> PathBuf {
    logs_dir(home).join(DAEMON_STDERR_LOG)
}

pub fn launch_agents_dir(home: &Path) -> PathBuf {
    home.join("Library").join("LaunchAgents")
}

pub fn launchd_plist_path(home: &Path) -> PathBuf {
    launch_agents_dir(home).join(format!("{DAEMON_LABEL}.plist"))
}
