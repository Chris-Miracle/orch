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
    std::fs::write(codebase.join("CLAUDE.md"), "legacy claude content").expect("write claude");
    std::fs::write(codebase.join(".cursor/rules/custom.mdc"), "legacy cursor rule").expect("write cursor");

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

    // Registry entry should exist
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

    // pilot.md, hidden guide, generated control files, imported legacy files, and backups should be generated
    assert!(codebase.join("orchestra/pilot.md").exists());
    assert!(codebase.join("orchestra/.guide.md").exists());
    assert!(codebase.join("orchestra/controls/CLAUDE.md").exists());
    assert!(codebase.join("orchestra/backup/manifest.json").exists());
    assert!(codebase.join("orchestra/backup/CLAUDE.md").exists());
    assert!(codebase.join("orchestra/backup/.cursor/rules/custom.mdc").exists());
    assert!(codebase.join("orchestra/controls/.cursor/rules/custom.mdc").exists());
    let imported_claude = std::fs::read_to_string(codebase.join("orchestra/controls/CLAUDE.md"))
        .expect("read imported claude");
    assert!(imported_claude.contains("legacy claude content"));

    // KEY: original user files must NOT be deleted — they coexist with the Orchestra control folder
    assert!(
        codebase.join("CLAUDE.md").exists(),
        "CLAUDE.md must be preserved in-place (not deleted by onboard)"
    );
    assert!(
        codebase.join(".cursor/rules/custom.mdc").exists(),
        ".cursor/rules/custom.mdc must be preserved in-place (not deleted by onboard)"
    );

    // Migration prompt should be printed (stdout contains the prompt header)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ORCHESTRA SETUP PROMPT") || stdout.contains("Mechanical migration"),
        "migration guidance should be printed to stdout"
    );
}

#[test]
fn onboard_mechanical_migrate_preserves_user_files() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("mechapp");

    std::fs::create_dir_all(&codebase).expect("mkdir");
    std::fs::write(codebase.join("CLAUDE.md"), "- Always write tests").expect("write claude");

    let binary = orchestra_bin_path();
    let output = Command::new(binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("onboard")
        .arg(&codebase)
        .arg("--project")
        .arg("mech")
        .arg("--yes")
        .arg("--migrate")
        .arg("mechanical")
        .output()
        .expect("run onboard mechanical");

    assert!(
        output.status.success(),
        "onboard mechanical failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // User files must remain and the new control folder should exist
    assert!(
        codebase.join("CLAUDE.md").exists(),
        "CLAUDE.md must be preserved after mechanical migrate"
    );
    assert!(codebase.join("orchestra/.guide.md").exists());
    assert!(codebase.join("orchestra/controls/CLAUDE.md").exists());
    let imported_claude = std::fs::read_to_string(codebase.join("orchestra/controls/CLAUDE.md"))
        .expect("read imported claude");
    assert!(imported_claude.contains("Always write tests"));

    // Mechanical mode should mention preservation
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mechanical migration"),
        "mechanical mode should print 'Mechanical migration' message"
    );
}

#[test]
fn onboard_imports_agent_library_tree_into_controls() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("agentlibapp");

    std::fs::create_dir_all(codebase.join("AGENT/.claude/agents")).expect("mkdir agent library");
    std::fs::write(codebase.join("AGENT/AGENTS.md"), "shared contract").expect("write shared contract");
    std::fs::write(codebase.join("AGENT/CLAUDE.md"), "claude entry").expect("write claude entry");
    std::fs::write(
        codebase.join("AGENT/.claude/agents/orchestra-worker.md"),
        "worker instructions",
    )
    .expect("write worker");

    let binary = orchestra_bin_path();
    let output = Command::new(binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("onboard")
        .arg(&codebase)
        .arg("--project")
        .arg("agentlib")
        .arg("--yes")
        .arg("--migrate")
        .arg("mechanical")
        .output()
        .expect("run onboard agentlib");

    assert!(
        output.status.success(),
        "onboard agent library failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(codebase.join("orchestra/backup/AGENT/AGENTS.md").exists());

    let agents = std::fs::read_to_string(codebase.join("orchestra/controls/AGENTS.md"))
        .expect("read merged AGENTS.md");
    assert!(agents.contains("shared contract"));

    let claude = std::fs::read_to_string(codebase.join("orchestra/controls/CLAUDE.md"))
        .expect("read merged CLAUDE.md");
    assert!(claude.contains("claude entry"));

    let worker = std::fs::read_to_string(
        codebase.join("orchestra/controls/.claude/agents/orchestra-worker.md"),
    )
    .expect("read merged worker");
    assert!(worker.contains("worker instructions"));
}

#[test]
fn onboard_delete_removes_legacy_agent_files_after_import() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("deleteapp");

    std::fs::create_dir_all(codebase.join("AGENT/.claude/agents")).expect("mkdir agent library");
    std::fs::write(codebase.join("AGENT/CLAUDE.md"), "claude entry").expect("write claude entry");
    std::fs::write(
        codebase.join("AGENT/prompt.txt"),
        "generic background note",
    )
    .expect("write prompt");

    let binary = orchestra_bin_path();
    let output = Command::new(binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("onboard")
        .arg(&codebase)
        .arg("--project")
        .arg("deleteproj")
        .arg("--yes")
        .arg("--migrate")
        .arg("mechanical")
        .arg("--delete")
        .output()
        .expect("run onboard delete");

    assert!(
        output.status.success(),
        "onboard delete failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!codebase.join("AGENT").exists(), "AGENT dir should be removed after --delete");

    let claude = std::fs::read_to_string(codebase.join("orchestra/controls/CLAUDE.md"))
        .expect("read merged CLAUDE.md");
    assert!(claude.contains("claude entry"));

    let guide = std::fs::read_to_string(codebase.join("orchestra/.guide.md"))
        .expect("read guide");
    assert!(guide.contains("generic background note"));
}

#[test]
fn onboard_imports_legacy_todos_into_registry_tasks() {
    let home = TempDir::new().expect("home");
    let workspace = TempDir::new().expect("workspace");
    let codebase = workspace.path().join("taskimportapp");

    std::fs::create_dir_all(&codebase).expect("mkdir");
    std::fs::write(
        codebase.join("CLAUDE.md"),
        "# Work\n- [ ] unify orchestration tasks\n- [x] remove stale prompt wording\n",
    )
    .expect("write legacy todo file");

    let binary = orchestra_bin_path();
    let output = Command::new(binary)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("onboard")
        .arg(&codebase)
        .arg("--project")
        .arg("taskimport")
        .arg("--yes")
        .arg("--migrate")
        .arg("mechanical")
        .output()
        .expect("run onboard task import");

    assert!(
        output.status.success(),
        "onboard task import failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let loaded = registry::load_codebase_at(
        home.path(),
        &ProjectName::from("taskimport"),
        &orchestra_core::types::CodebaseName::from("taskimportapp"),
    )
    .expect("load registered codebase");

    assert_eq!(loaded.projects[0].tasks.len(), 2);
    assert!(loaded.projects[0]
        .tasks
        .iter()
        .any(|task| task.title == "unify orchestration tasks" && matches!(task.status, orchestra_core::types::TaskStatus::Pending)));
    assert!(loaded.projects[0]
        .tasks
        .iter()
        .any(|task| task.title == "remove stale prompt wording" && matches!(task.status, orchestra_core::types::TaskStatus::Done)));
}

