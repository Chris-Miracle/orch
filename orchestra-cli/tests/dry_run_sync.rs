use orchestra_core::{
    registry,
    types::{ProjectName, ProjectType},
};
use tempfile::TempDir;

fn orchestra_bin_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_orchestra") {
        return std::path::PathBuf::from(path);
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
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else { return false };
            name.starts_with("orchestra-")
                && !name.ends_with(".d")
                && p.is_file()
        })
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .next()
        .expect("unable to locate orchestra binary in target/debug or target/debug/deps")
}

#[test]
fn dry_run_sync_reports_files_and_writes_nothing() {
    let home = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let codebase_dir = workspace.path().join("copnow_api");
    std::fs::create_dir_all(&codebase_dir).unwrap();

    registry::init_at(
        codebase_dir.clone(),
        ProjectName::from("copnow"),
        Some(ProjectType::Backend),
        home.path(),
    )
    .expect("init");

    let output = std::process::Command::new(orchestra_bin_path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("sync")
        .arg("copnow_api")
        .arg("--dry-run")
        .output()
        .expect("run orchestra sync --dry-run");
    assert!(
        output.status.success(),
        "command failed: status={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("[dry-run]"), "missing dry-run prefix");
    assert!(stdout.contains("CLAUDE.md"), "missing CLAUDE.md in output");
    assert!(stdout.contains("settings.json"), "missing settings.json in output");

    let mut entries = std::fs::read_dir(&codebase_dir).unwrap();
    assert!(entries.next().is_none(), "dry-run must not create files");
}
