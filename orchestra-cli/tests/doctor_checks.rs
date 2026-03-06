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
fn doctor_json_contains_expected_checks() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("doctor_app");
    std::fs::create_dir_all(&codebase).expect("mkdir codebase");

    registry::init_at(
        codebase,
        ProjectName::from("acme"),
        None,
        home.path(),
    )
    .expect("init");

    let binary = orchestra_bin_path();
    let output = Command::new(binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");

    let checks = json
        .get("checks")
        .and_then(|v| v.as_array())
        .expect("checks array");
    let names: Vec<&str> = checks
        .iter()
        .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(names.contains(&"version update"));
    assert!(names.contains(&"daemon status"));
    assert!(names.contains(&"registry integrity"));
    assert!(names.contains(&"codebase paths"));
    assert!(names.contains(&"pilot.md presence"));
    assert!(names.contains(&"guide presence"));
    assert!(names.contains(&"staleness summary"));
}
