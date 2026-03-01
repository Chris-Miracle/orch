use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use orchestra_core::{
    registry,
    types::{CodebaseName, ProjectName, ProjectType},
};
use tempfile::TempDir;

fn orchestra_bin_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_orchestra") {
        return PathBuf::from(path);
    }

    let this_test = std::env::current_exe().expect("current_exe");
    let deps_dir = this_test.parent().expect("deps dir");
    let debug_dir = deps_dir.parent().expect("debug dir");

    let direct = {
        #[cfg(windows)]
        {
            debug_dir.join("orchestra.exe")
        }
        #[cfg(not(windows))]
        {
            debug_dir.join("orchestra")
        }
    };
    if direct.exists() {
        return direct;
    }

    let mut candidates: Vec<_> = std::fs::read_dir(deps_dir)
        .expect("read deps dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            name.starts_with("orchestra-") && !name.ends_with(".d") && p.is_file()
        })
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .next()
        .expect("unable to locate orchestra binary in target/debug or target/debug/deps")
}

struct DaemonProcess {
    child: Child,
    binary: PathBuf,
    home: PathBuf,
}

impl DaemonProcess {
    fn start(binary: PathBuf, home: PathBuf) -> Self {
        let child = Command::new(&binary)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .args(["daemon", "start"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn daemon");

        Self {
            child,
            binary,
            home,
        }
    }

    fn stop(&mut self) {
        let _ = Command::new(&self.binary)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .args(["daemon", "stop"])
            .status();

        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            sleep(Duration::from_millis(50));
        }

        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

fn daemon_running(binary: &Path, home: &Path) -> bool {
    let output = match Command::new(binary)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .args(["daemon", "status"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }

    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return false;
    };
    value
        .get("running")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn registry_change_triggers_auto_sync() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase_dir = workspace.path().join("copnow_api");
    std::fs::create_dir_all(&codebase_dir).expect("mkdir codebase");

    let binary = orchestra_bin_path();
    let mut daemon = DaemonProcess::start(binary.clone(), home.path().to_path_buf());
    assert!(
        wait_until(Duration::from_secs(5), || daemon_running(
            &binary,
            home.path()
        )),
        "daemon did not report running state in time",
    );

    let project = ProjectName::from("copnow");
    let codebase_name = CodebaseName::from("copnow_api");
    registry::init_at(
        codebase_dir.clone(),
        project.clone(),
        Some(ProjectType::Backend),
        home.path(),
    )
    .expect("init codebase");

    let sync_output = Command::new(&binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .args(["sync", "copnow_api"])
        .output()
        .expect("run baseline sync");
    assert!(
        sync_output.status.success(),
        "baseline sync failed: {}",
        String::from_utf8_lossy(&sync_output.stderr),
    );

    let target = codebase_dir.join("CLAUDE.md");
    assert!(
        target.exists(),
        "CLAUDE.md should exist after baseline sync"
    );

    let sentinel = "phase04-autosync-sentinel";
    let mut codebase =
        registry::load_codebase_at(home.path(), &project, &codebase_name).expect("load codebase");
    codebase.projects[0].name = ProjectName::from(sentinel);
    registry::save_codebase_at(home.path(), &project, &codebase).expect("save codebase");

    let synced = wait_until(Duration::from_secs(10), || {
        std::fs::read_to_string(&target)
            .map(|content| content.contains(sentinel))
            .unwrap_or(false)
    });

    assert!(
        synced,
        "daemon did not auto-sync updated registry content within timeout",
    );

    daemon.stop();
}
