//! Registry error-message, atomic-write-safety, and init integration tests.

use assert_fs::prelude::*;
use chrono::Utc;
use orchestra_core::{
    registry,
    types::{ProjectName, ProjectType, Registry},
    RegistryError,
};
use predicates::prelude::predicate;
use std::fs;

// ---------------------------------------------------------------------------
// 1. Registry load error messages
// ---------------------------------------------------------------------------

#[test]
fn load_missing_registry_returns_not_found() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let err = registry::load_at(home.path()).unwrap_err();
    assert!(
        matches!(err, RegistryError::RegistryNotFound { .. }),
        "expected RegistryNotFound, got: {err}"
    );
    assert!(err.to_string().contains("registry not found"));
    assert!(err.to_string().contains(".orchestra/registry.yaml"));
}

#[test]
fn load_corrupt_yaml_returns_parse_error_with_path() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    // Set up the .orchestra dir and write corrupt YAML directly
    let orch_dir = home.path().join(".orchestra");
    fs::create_dir_all(&orch_dir).expect("mkdir");
    let registry_file = orch_dir.join("registry.yaml");
    fs::write(&registry_file, b": : : corrupt : yaml : !!!\n  - broken: [unclosed").expect("write");

    let err = registry::load_at(home.path()).unwrap_err();
    assert!(
        matches!(err, RegistryError::Parse { .. }),
        "expected Parse error, got: {err}"
    );
    let msg = err.to_string();
    // Must contain the file path
    assert!(
        msg.contains("registry.yaml"),
        "error message must contain file path, got: {msg}"
    );
    // serde_yaml embeds line info in its Display — verify it's present
    // (The inner source message from serde_yaml includes "line X" or similar)
    let source_msg = match &err {
        RegistryError::Parse { source, .. } => source.to_string(),
        _ => unreachable!(),
    };
    // serde_yaml error messages contain "line" in their output
    assert!(
        source_msg.contains("line") || source_msg.contains("expected") || !source_msg.is_empty(),
        "serde_yaml error must have context, got: {source_msg}"
    );
}

#[test]
fn load_wrong_type_yaml_returns_parse_error() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let orch_dir = home.path().join(".orchestra");
    fs::create_dir_all(&orch_dir).expect("mkdir");
    fs::write(orch_dir.join("registry.yaml"), b"- this is a list, not a mapping\n").expect("write");

    let err = registry::load_at(home.path()).unwrap_err();
    assert!(matches!(err, RegistryError::Parse { .. }), "expected Parse, got: {err}");
    assert!(err.to_string().contains("registry.yaml"));
}

// ---------------------------------------------------------------------------
// 2. Atomic write safety
// ---------------------------------------------------------------------------

#[test]
fn save_cleans_up_tmp_file() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let now = Utc::now();
    let reg = Registry { version: 1, codebases: vec![], created_at: now, updated_at: now };
    registry::save_at(&reg, home.path()).expect("save");

    let tmp = registry::registry_path_at(home.path()).with_file_name("registry.yaml.tmp");
    assert!(!tmp.exists(), ".tmp must not exist after successful save");
}

#[test]
fn mid_write_crash_leaves_original_intact() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    // Write initial valid registry
    let now = Utc::now();
    let original = Registry { version: 1, codebases: vec![], created_at: now, updated_at: now };
    registry::save_at(&original, home.path()).expect("initial save");

    let registry_path = registry::registry_path_at(home.path());
    let original_bytes = fs::read(&registry_path).expect("read original");

    // Simulate crash: write .tmp but do NOT rename (process "died" before rename)
    let tmp_path = registry_path.with_file_name("registry.yaml.tmp");
    fs::write(&tmp_path, b"CRASH - INCOMPLETE WRITE").expect("write tmp");

    // Original must be unchanged
    let current_bytes = fs::read(&registry_path).expect("read after crash");
    assert_eq!(original_bytes, current_bytes, "original file must be unchanged after crash");
}

#[test]
fn save_and_load_roundtrip_via_home() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let now = Utc::now();
    let reg = Registry { version: 1, codebases: vec![], created_at: now, updated_at: now };
    registry::save_at(&reg, home.path()).expect("save");

    let loaded = registry::load_at(home.path()).expect("load");
    assert_eq!(reg.version, loaded.version);
    assert_eq!(reg.codebases, loaded.codebases);
}

// ---------------------------------------------------------------------------
// 3. Init integration test
// ---------------------------------------------------------------------------

#[test]
fn init_creates_registry_with_correct_content() {
    let home = assert_fs::TempDir::new().expect("home tempdir");
    let codebase = assert_fs::TempDir::new().expect("codebase tempdir");

    let registry = registry::init_at(
        codebase.path().to_path_buf(),
        ProjectName::from("myproject"),
        Some(ProjectType::Backend),
        home.path(),
    )
    .expect("init");

    // Registry file must exist
    let registry_file = home.path().join(".orchestra").join("registry.yaml");
    home.child(".orchestra/registry.yaml").assert(predicate::path::exists());

    // File must be valid YAML that roundtrips
    let contents = fs::read_to_string(&registry_file).expect("read");
    let loaded: Registry = serde_yaml::from_str(&contents).expect("roundtrip deserialize");
    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.codebases.len(), 1);
    assert_eq!(loaded.codebases[0].projects[0].name, ProjectName::from("myproject"));
    assert_eq!(loaded.codebases[0].projects[0].project_type, ProjectType::Backend);

    // Returned registry must match loaded
    assert_eq!(registry.codebases.len(), loaded.codebases.len());

    // Unix: file permissions must be 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&registry_file).expect("metadata");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "registry.yaml must have mode 0600, got: {mode:o}");
    }
}

#[test]
fn init_is_idempotent_for_same_codebase() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let codebase = assert_fs::TempDir::new().expect("tempdir");

    registry::init_at(
        codebase.path().to_path_buf(),
        ProjectName::from("proj"),
        None,
        home.path(),
    )
    .expect("first init");

    registry::init_at(
        codebase.path().to_path_buf(),
        ProjectName::from("proj"),
        None,
        home.path(),
    )
    .expect("second init");

    let loaded = registry::load_at(home.path()).expect("load");
    assert_eq!(loaded.codebases.len(), 1, "same codebase must not be duplicated");
}

#[test]
fn add_project_appends_to_registry() {
    let home = assert_fs::TempDir::new().expect("tempdir");
    let codebase = assert_fs::TempDir::new().expect("tempdir");

    registry::init_at(
        codebase.path().to_path_buf(),
        ProjectName::from("api"),
        Some(ProjectType::Backend),
        home.path(),
    )
    .expect("init");

    registry::add_project_at(
        ProjectName::from("dashboard"),
        ProjectType::Frontend,
        home.path(),
    )
    .expect("add_project");

    let loaded = registry::load_at(home.path()).expect("load");
    assert_eq!(loaded.codebases[0].projects.len(), 2);
    assert_eq!(loaded.codebases[0].projects[1].name, ProjectName::from("dashboard"));
}
