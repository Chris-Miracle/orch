use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn orchestra_bin_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_orchestra") {
        return PathBuf::from(path);
    }

    let this_test = std::env::current_exe().expect("current_exe");
    let deps_dir = this_test.parent().expect("deps dir");
    let debug_dir = deps_dir.parent().expect("debug dir");

    let direct = debug_dir.join("orchestra");
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
        .expect("unable to locate orchestra binary")
}

#[test]
fn offboard_recent_restores_original_files_and_removes_orchestra_dir() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("recover_app");

    std::fs::create_dir_all(codebase.join(".cursor/rules")).expect("mkdir");
    std::fs::write(codebase.join("CLAUDE.md"), "legacy claude content\n").expect("write claude");
    std::fs::write(codebase.join(".cursor/rules/custom.mdc"), "legacy cursor rule\n")
        .expect("write cursor");

    let binary = orchestra_bin_path();
    let onboard = Command::new(&binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("onboard")
        .arg(&codebase)
        .arg("--project")
        .arg("acme")
        .arg("--yes")
        .output()
        .expect("run onboard");
    assert!(
        onboard.status.success(),
        "onboard failed: {}",
        String::from_utf8_lossy(&onboard.stderr)
    );

    let offboard = Command::new(&binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("offboard")
        .arg(&codebase)
        .arg("--yes")
        .arg("--recent")
        .output()
        .expect("run offboard");
    assert!(
        offboard.status.success(),
        "offboard failed: {}",
        String::from_utf8_lossy(&offboard.stderr)
    );

    assert!(!codebase.join("orchestra").exists());
    assert_eq!(
        std::fs::read_to_string(codebase.join("CLAUDE.md")).expect("read restored claude"),
        "legacy claude content\n"
    );
    assert_eq!(
        std::fs::read_to_string(codebase.join(".cursor/rules/custom.mdc")).expect("read restored cursor"),
        "legacy cursor rule\n"
    );

    let stdout = String::from_utf8_lossy(&offboard.stdout);
    assert!(stdout.contains("revert recent onboarding") || stdout.contains("Restored"));
}

#[test]
fn reset_restore_backups_removes_project_orchestra_dirs_and_global_state() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("reset_app");

    std::fs::create_dir_all(&codebase).expect("mkdir");
    std::fs::write(codebase.join("CLAUDE.md"), "legacy reset content\n").expect("write claude");

    let binary = orchestra_bin_path();
    let onboard = Command::new(&binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("onboard")
        .arg(&codebase)
        .arg("--project")
        .arg("acme")
        .arg("--yes")
        .arg("--migrate")
        .arg("mechanical")
        .output()
        .expect("run onboard");
    assert!(
        onboard.status.success(),
        "onboard failed: {}",
        String::from_utf8_lossy(&onboard.stderr)
    );

    let reset = Command::new(&binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("reset")
        .arg("--confirm")
        .arg("--restore-backups")
        .output()
        .expect("run reset");
    assert!(
        reset.status.success(),
        "reset failed: {}",
        String::from_utf8_lossy(&reset.stderr)
    );

    assert!(!codebase.join("orchestra").exists());
    assert_eq!(
        std::fs::read_to_string(codebase.join("CLAUDE.md")).expect("read restored file"),
        "legacy reset content\n"
    );
    assert!(!home.path().join(".orchestra").exists());

    let stdout = String::from_utf8_lossy(&reset.stdout);
    assert!(stdout.contains("To reinstall the latest version"));
}