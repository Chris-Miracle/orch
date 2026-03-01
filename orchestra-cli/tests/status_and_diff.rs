use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use assert_cmd::prelude::*;
use predicates::str::contains;

use orchestra_core::{
    registry,
    types::{CodebaseName, ProjectName, ProjectType},
};
use tempfile::TempDir;

fn orchestra_cmd(home: &Path) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("orchestra"));
    cmd.env("HOME", home).env("USERPROFILE", home);
    cmd
}

fn init_codebase(
    home: &TempDir,
    workspace: &TempDir,
    project: &ProjectName,
    codebase_name: &str,
) -> PathBuf {
    let codebase_dir = workspace.path().join(codebase_name);
    fs::create_dir_all(&codebase_dir).expect("create codebase dir");
    registry::init_at(
        codebase_dir.clone(),
        project.clone(),
        Some(ProjectType::Backend),
        home.path(),
    )
    .expect("init codebase");
    codebase_dir
}

fn sync_codebase_via_cli(home: &TempDir, codebase_name: &str) {
    orchestra_cmd(home.path())
        .args(["sync", codebase_name])
        .assert()
        .success();
}

#[test]
fn diff_accuracy_on_registry_change() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let project = ProjectName::from("copnow");

    init_codebase(&home, &workspace, &project, "copnow_api");
    sync_codebase_via_cli(&home, "copnow_api");

    // Simulate a "convention" change by mutating registry-backed project metadata
    // with a unique sentinel that must appear as an added line in diff output.
    let sentinel = "convention-sentinel-phase03";
    let codebase_name = CodebaseName::from("copnow_api");
    let mut codebase =
        registry::load_codebase_at(home.path(), &project, &codebase_name).expect("load codebase");
    codebase.projects[0].name = ProjectName::from(sentinel);
    registry::save_codebase_at(home.path(), &project, &codebase).expect("save codebase");

    let assert = orchestra_cmd(home.path())
        .args(["diff", "copnow_api"])
        .assert()
        .success()
        .stdout(contains(sentinel));
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");

    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with('+') && line.contains(sentinel)),
        "expected a unified diff added line for registry convention change"
    );
    assert!(
        !stdout
            .lines()
            .any(|line| (line.starts_with('+') || line.starts_with('-'))
                && line.contains("last_synced")),
        "diff should not include unrelated last_synced metadata noise"
    );
}

#[test]
fn status_json_includes_all_codebases_with_expected_staleness_and_schema() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let project = ProjectName::from("copnow");

    let current_dir = init_codebase(&home, &workspace, &project, "current_api");
    let modified_dir = init_codebase(&home, &workspace, &project, "modified_api");
    let stale_dir = init_codebase(&home, &workspace, &project, "stale_api");
    let _never_dir = init_codebase(&home, &workspace, &project, "never_api");

    sync_codebase_via_cli(&home, "current_api");
    sync_codebase_via_cli(&home, "modified_api");
    sync_codebase_via_cli(&home, "stale_api");

    fs::write(modified_dir.join("CLAUDE.md"), "manual local change\n").expect("modify file");

    sleep(Duration::from_millis(1100));
    let stale_registry =
        registry::codebase_path_at(home.path(), &project, &CodebaseName::from("stale_api"));
    let stale_yaml = fs::read_to_string(&stale_registry).expect("read stale registry");
    fs::write(&stale_registry, stale_yaml).expect("touch stale registry");

    assert!(
        current_dir.join("CLAUDE.md").exists(),
        "current codebase should be synced"
    );
    assert!(
        stale_dir.join("CLAUDE.md").exists(),
        "stale codebase should be synced"
    );

    let assert = orchestra_cmd(home.path())
        .args(["status", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8");
    let payload: serde_json::Value = serde_json::from_str(&stdout).expect("parse status json");

    let top_keys: BTreeSet<String> = payload
        .as_object()
        .expect("status root object")
        .keys()
        .cloned()
        .collect();
    let expected_top: BTreeSet<String> = ["summary", "codebases"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(top_keys, expected_top, "status root schema changed");

    let summary_keys: BTreeSet<String> = payload["summary"]
        .as_object()
        .expect("summary object")
        .keys()
        .cloned()
        .collect();
    let expected_summary: BTreeSet<String> = ["projects", "codebases", "stale"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(summary_keys, expected_summary, "summary schema changed");

    let rows = payload["codebases"].as_array().expect("codebases array");
    assert_eq!(rows.len(), 4, "expected all codebases in status output");

    let expected_row_fields: BTreeSet<String> = [
        "project",
        "codebase",
        "status",
        "detail",
        "last_sync_age",
        "last_sync_at",
        "active_tasks",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    let mut by_name = HashMap::new();
    for row in rows {
        let object = row.as_object().expect("row object");
        let keys: BTreeSet<String> = object.keys().cloned().collect();
        assert_eq!(keys, expected_row_fields, "codebase row schema changed");

        let name = row["codebase"].as_str().expect("codebase name").to_string();
        let status = row["status"].as_str().expect("status").to_string();
        by_name.insert(name, status);
    }

    assert_eq!(
        by_name.get("current_api").map(String::as_str),
        Some("current")
    );
    assert_eq!(
        by_name.get("modified_api").map(String::as_str),
        Some("modified")
    );
    assert_eq!(by_name.get("stale_api").map(String::as_str), Some("stale"));
    assert_eq!(
        by_name.get("never_api").map(String::as_str),
        Some("never_synced")
    );
}
