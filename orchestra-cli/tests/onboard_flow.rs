use std::path::PathBuf;
use std::process::Command;

use orchestra_core::{registry, types::ProjectName};
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
fn onboard_registers_backups_and_generates_pilot() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("myapp");

    std::fs::create_dir_all(codebase.join(".cursor/rules")).expect("mkdir cursor");
    std::fs::write(codebase.join("CLAUDE.md"), "legacy").expect("write claude");
    std::fs::write(codebase.join(".cursor/rules/custom.mdc"), "legacy").expect("write cursor");

    let binary = orchestra_bin_path();
    let output = Command::new(binary)
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
        output.status.success(),
        "onboard failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let loaded = registry::load_codebase_at(
        home.path(),
        &ProjectName::from("acme"),
        &orchestra_core::types::CodebaseName::from("myapp"),
    )
    .expect("load registered codebase");
    assert_eq!(
        loaded.path.canonicalize().expect("canonical loaded path"),
        codebase.canonicalize().expect("canonical expected path")
    );

    assert!(codebase.join(".orchestra/pilot.md").exists());
    assert!(codebase.join(".orchestra/backup/manifest.json").exists());
    assert!(codebase.join(".orchestra/backup/CLAUDE.md").exists());
    assert!(codebase.join(".orchestra/backup/.cursor/rules/custom.mdc").exists());
}
