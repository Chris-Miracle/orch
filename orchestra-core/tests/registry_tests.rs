//! Registry error-message, atomic-write-safety, and init integration tests.
//! Migrated for per-codebase storage: ~/.orchestra/projects/<project>/<codebase>.yaml

use assert_fs::prelude::*;
use chrono::Utc;
use orchestra_core::{
    registry,
    types::{CodebaseName, ProjectName, ProjectType},
    RegistryError,
};
use predicates::prelude::predicate;
use std::fs;

fn proj() -> ProjectName { ProjectName::from("copnow") }
fn cb() -> CodebaseName { CodebaseName::from("copnow_api") }

// ---------------------------------------------------------------------------
// 1. Load error messages
// ---------------------------------------------------------------------------

#[test]
fn load_missing_codebase_returns_not_found() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let err = registry::load_codebase_at(home.path(), &proj(), &cb()).unwrap_err();
    assert!(matches!(err, RegistryError::RegistryNotFound { .. }), "got: {err}");
    assert!(err.to_string().contains("registry not found"));
    assert!(err.to_string().contains("copnow_api.yaml"));
}

#[test]
fn load_corrupt_yaml_returns_parse_error_with_path() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let dir = home.path().join(".orchestra").join("projects").join("copnow");
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("copnow_api.yaml"), b": : corrupt : yaml : !!!\n  - broken: [unclosed")
        .expect("write");

    let err = registry::load_codebase_at(home.path(), &proj(), &cb()).unwrap_err();
    assert!(matches!(err, RegistryError::Parse { .. }), "got: {err}");
    let msg = err.to_string();
    assert!(msg.contains("copnow_api.yaml"), "must contain file path, got: {msg}");
    let source_msg = match &err {
        RegistryError::Parse { source, .. } => source.to_string(),
        _ => unreachable!(),
    };
    assert!(!source_msg.is_empty(), "serde_yaml must provide error context");
}

#[test]
fn load_wrong_type_yaml_returns_parse_error() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let dir = home.path().join(".orchestra").join("projects").join("copnow");
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("copnow_api.yaml"), b"- this is a list, not a mapping\n").expect("write");

    let err = registry::load_codebase_at(home.path(), &proj(), &cb()).unwrap_err();
    assert!(matches!(err, RegistryError::Parse { .. }), "got: {err}");
}

// ---------------------------------------------------------------------------
// 2. Atomic write safety
// ---------------------------------------------------------------------------

#[test]
fn save_cleans_up_tmp_file() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let now = Utc::now();
    let codebase = orchestra_core::types::Codebase {
        name: cb(),
        path: std::path::PathBuf::from("/code/copnow_api"),
        projects: vec![],
        created_at: now,
        updated_at: now,
    };
    registry::save_codebase_at(home.path(), &proj(), &codebase).expect("save");

    let yaml_path = registry::codebase_path_at(home.path(), &proj(), &cb());
    let tmp = yaml_path.with_file_name("copnow_api.yaml.tmp");
    assert!(!tmp.exists(), ".tmp must be removed after successful save");
}

#[test]
fn mid_write_crash_leaves_original_intact() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let now = Utc::now();
    let codebase = orchestra_core::types::Codebase {
        name: cb(),
        path: std::path::PathBuf::from("/code/copnow_api"),
        projects: vec![],
        created_at: now,
        updated_at: now,
    };
    registry::save_codebase_at(home.path(), &proj(), &codebase).expect("save");

    let yaml_path = registry::codebase_path_at(home.path(), &proj(), &cb());
    let original_bytes = fs::read(&yaml_path).expect("read original");

    // Simulate crash: .tmp written but process died before rename
    let tmp = yaml_path.with_file_name("copnow_api.yaml.tmp");
    fs::write(&tmp, b"CRASH - INCOMPLETE WRITE").expect("write crash tmp");

    let current_bytes = fs::read(&yaml_path).expect("read after crash");
    assert_eq!(original_bytes, current_bytes, "original must be unchanged after crash");
    assert!(tmp.exists(), ".tmp orphan must exist (crash = no cleanup)");
}

// ---------------------------------------------------------------------------
// 3. Init integration test
// ---------------------------------------------------------------------------

#[test]
fn init_creates_per_codebase_yaml() {
    let home = assert_fs::TempDir::new().expect("home tempdir");
    let codebase_dir = assert_fs::TempDir::new().expect("codebase tempdir");

    let codebase = registry::init_at(
        codebase_dir.path().to_path_buf(),
        proj(),
        Some(ProjectType::Backend),
        home.path(),
    )
    .expect("init");

    // File must exist at correct path
    let expected_rel = format!(
        ".orchestra/projects/copnow/{}.yaml",
        codebase_dir.path().file_name().unwrap().to_string_lossy()
    );
    home.child(&expected_rel).assert(predicate::path::exists());

    // File content must roundtrip
    let yaml_path = registry::codebase_path_at(home.path(), &proj(), &codebase.name);
    let contents = fs::read_to_string(&yaml_path).expect("read");
    let loaded: orchestra_core::types::Codebase =
        serde_yaml::from_str(&contents).expect("roundtrip");
    assert_eq!(loaded.name, codebase.name);
    assert_eq!(loaded.projects[0].project_type, ProjectType::Backend);

    // Unix: mode 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&yaml_path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }
}

#[test]
fn project_index_created_on_init() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let codebase_dir = assert_fs::TempDir::new().expect("tempdir");

    registry::init_at(codebase_dir.path().to_path_buf(), proj(), None, home.path())
        .expect("init");

    home.child(".orchestra/projects/copnow/project.yaml")
        .assert(predicate::path::exists());
}

#[test]
fn init_is_idempotent() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let cb_dir = assert_fs::TempDir::new().expect("tempdir");

    registry::init_at(cb_dir.path().to_path_buf(), proj(), Some(ProjectType::Backend), home.path())
        .expect("first init");
    registry::init_at(cb_dir.path().to_path_buf(), proj(), Some(ProjectType::Frontend), home.path())
        .expect("second init");

    // Only one file, type unchanged (first wins — idempotent)
    let list = registry::list_codebases_at(home.path()).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].1.projects[0].project_type, ProjectType::Backend);
}

// ---------------------------------------------------------------------------
// 4. Multiple codebases and list
// ---------------------------------------------------------------------------

#[test]
fn list_codebases_groups_by_project() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let api_dir = assert_fs::TempDir::new().expect("tempdir");
    let mobile_dir = assert_fs::TempDir::new().expect("tempdir");

    registry::init_at(api_dir.path().to_path_buf(), proj(), Some(ProjectType::Backend), home.path())
        .expect("init api");
    registry::init_at(mobile_dir.path().to_path_buf(), proj(), Some(ProjectType::Mobile), home.path())
        .expect("init mobile");

    let list = registry::list_codebases_at(home.path()).expect("list");
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|(p, _)| p == &proj()));
}

#[test]
fn add_codebase_creates_new_yaml() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let cb_dir = assert_fs::TempDir::new().expect("tempdir");

    registry::init_at(cb_dir.path().to_path_buf(), proj(), Some(ProjectType::Backend), home.path())
        .expect("init");

    registry::add_codebase_at(
        home.path(),
        &proj(),
        CodebaseName::from("copnow_mobile"),
        ProjectType::Mobile,
    )
    .expect("add");

    let list = registry::list_codebases_at(home.path()).expect("list");
    assert_eq!(list.len(), 2);

    home.child(".orchestra/projects/copnow/copnow_mobile.yaml")
        .assert(predicate::path::exists());
}

#[test]
fn add_codebase_to_nonexistent_project_errors() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let err = registry::add_codebase_at(
        home.path(),
        &proj(),
        CodebaseName::from("anything"),
        ProjectType::Backend,
    )
    .unwrap_err();
    assert!(matches!(err, RegistryError::RegistryNotFound { .. }));
}

#[test]
fn list_is_sorted_and_deterministic() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let alpha = assert_fs::TempDir::new().expect("tempdir");
    let beta = assert_fs::TempDir::new().expect("tempdir");

    let proj_b = ProjectName::from("beta_project");
    let proj_a = ProjectName::from("alpha_project");

    // Register beta before alpha intentionally
    registry::init_at(beta.path().to_path_buf(), proj_b.clone(), None, home.path()).expect("beta");
    registry::init_at(alpha.path().to_path_buf(), proj_a.clone(), None, home.path()).expect("alpha");

    let list = registry::list_codebases_at(home.path()).expect("list");
    assert_eq!(list.len(), 2);
    // alpha_project must come first (sorted)
    assert_eq!(list[0].0, proj_a);
    assert_eq!(list[1].0, proj_b);
}
