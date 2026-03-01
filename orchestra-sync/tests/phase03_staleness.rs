use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use filetime::{set_file_mtime, FileTime};
use orchestra_core::{
    registry,
    types::{Codebase, ProjectName, ProjectType},
};
use orchestra_renderer::AgentKind;
use orchestra_sync::{
    staleness::{check, StalenessSignal},
    sync_codebase,
};
use tempfile::TempDir;

fn init_codebase(
    home: &TempDir,
    workspace: &TempDir,
    project: &ProjectName,
    codebase_name: &str,
    do_sync: bool,
) -> Codebase {
    let codebase_dir = workspace.path().join(codebase_name);
    fs::create_dir_all(&codebase_dir).expect("create codebase dir");
    registry::init_at(
        codebase_dir,
        project.clone(),
        Some(ProjectType::Backend),
        home.path(),
    )
    .expect("init");

    if do_sync {
        sync_codebase(codebase_name, home.path(), false).expect("sync");
    }

    registry::list_codebases_at(home.path())
        .expect("list codebases")
        .into_iter()
        .find(|(_, cb)| cb.name.0 == codebase_name)
        .map(|(_, cb)| cb)
        .expect("codebase present")
}

#[test]
fn stale_when_registry_is_newer_than_managed_files() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let project = ProjectName::from("copnow");
    let codebase = init_codebase(&home, &workspace, &project, "copnow_api", true);

    let old = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(24 * 60 * 60));
    for agent in AgentKind::all() {
        for path in agent.output_paths(&codebase.path) {
            set_file_mtime(&path, old).expect("set old mtime for agent file");
        }
    }

    let registry_path = registry::codebase_path_at(home.path(), &project, &codebase.name);
    let new = FileTime::from_system_time(SystemTime::now() + Duration::from_secs(120));
    set_file_mtime(&registry_path, new).expect("touch registry mtime");

    let signal = check(home.path(), &project, &codebase).expect("check");
    match signal {
        StalenessSignal::Stale { .. } => {}
        other => panic!("expected stale, got {other:?}"),
    }
}

#[test]
fn modified_when_hash_mismatch_detected() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let project = ProjectName::from("copnow");
    let codebase = init_codebase(&home, &workspace, &project, "copnow_api", true);

    let claude = codebase.path.join("CLAUDE.md");
    fs::write(&claude, "manually changed content\n").expect("edit CLAUDE.md");

    let signal = check(home.path(), &project, &codebase).expect("check");
    match signal {
        StalenessSignal::Modified { files } => {
            assert!(
                files.contains(&PathBuf::from("CLAUDE.md")),
                "modified set should include CLAUDE.md"
            );
        }
        other => panic!("expected modified, got {other:?}"),
    }
}

#[test]
fn current_immediately_after_sync() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let project = ProjectName::from("copnow");
    let codebase = init_codebase(&home, &workspace, &project, "copnow_api", true);

    let signal = check(home.path(), &project, &codebase).expect("check");
    assert_eq!(signal, StalenessSignal::Current);
}

#[test]
fn never_synced_when_registry_exists_but_no_hash_store() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let project = ProjectName::from("copnow");
    let codebase = init_codebase(&home, &workspace, &project, "copnow_api", false);

    let signal = check(home.path(), &project, &codebase).expect("check");
    assert_eq!(signal, StalenessSignal::NeverSynced);
}
