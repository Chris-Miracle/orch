//! Phase 05 writeback protocol — top-level orchestrator.
//!
//! The entry point is [`process_writeback`]. It is called by the daemon
//! watcher when an agent file change is detected.

pub mod applier;
pub mod log;
pub mod parser;
pub mod strip;
pub mod types;

use std::path::Path;

use chrono::Utc;
use orchestra_core::registry;
use orchestra_renderer::engine::{guide_path, pilot_path};
use orchestra_renderer::AgentKind;

use crate::{
    error::SyncError,
    hash_store,
    pipeline,
    pipeline::SyncScope,
    writeback::types::WritebackCommand,
};

use self::log::{log_event, WritebackEvent};
use self::strip::{strip_update_block, write_error_block_messages};
use self::types::ApplyOutcome;
pub use self::types::WritebackOutcome;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Process a potential writeback in `agent_file`.
///
/// Returns `WritebackOutcome::no_block()` quickly if no update block is found.
///
/// Full cycle when a block is found:
/// 1. Parse block → commands + parse errors
/// 2. Identify which codebase owns `agent_file` (walk registry)
/// 3. Load codebase, apply commands, save registry
/// 4. Strip the update block (atomic)
/// 5. Write error block if there were parse errors
/// 6. Run full sync pipeline for that codebase
/// 7. Log the event
pub fn process_writeback(home: &Path, agent_file: &Path) -> Result<WritebackOutcome, SyncError> {
    // 1. Read file
    let content = std::fs::read_to_string(agent_file).map_err(|e| {
        crate::error::io_err(agent_file, e)
    })?;

    let update_block = parser::find_update_block(&content);
    let task_block = parser::find_task_block(&content);

    if update_block.is_none() && task_block.is_none() {
        return Ok(WritebackOutcome::no_block());
    }

    let parse_result = if let Some(block_meta) = update_block.as_ref() {
        let total_lines = content.lines().count();
        if !block_meta.in_allowed_window(total_lines) {
            tracing::debug!(
                "writeback block ignored: outside allowed top/bottom windows path={} start_line={} end_line={} total_lines={}",
                agent_file.display(),
                block_meta.start_line,
                block_meta.end_line,
                total_lines
            );
            return Ok(WritebackOutcome::no_block());
        }
        parser::parse_block(block_meta.content)
    } else {
        self::types::ParseResult {
            commands: vec![],
            errors: vec![],
        }
    };

    let task_parse_result = if let Some(block_meta) = task_block.as_ref() {
        parser::parse_task_block(block_meta.content)
    } else {
        self::types::TaskParseResult {
            tasks: vec![],
            errors: vec![],
        }
    };

    // 3. Find the codebase that owns this agent file
    let all = registry::list_codebases_at(home)?;
    let owning_codebase = find_owning_codebase(&all, agent_file, &parse_result.commands);

    let (project_name, mut codebase) = match owning_codebase {
        Some(pair) => pair,
        None => {
            tracing::warn!(
                "writeback: could not identify codebase for agent file: {}",
                agent_file.display()
            );

            let mut teaching_messages = parse_result
                .errors
                .iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>();
            teaching_messages.extend(task_parse_result.errors.iter().map(|error| error.to_string()));
            teaching_messages.push("File not associated with any registered codebase".to_owned());

            let block_stripped = match strip_update_block(agent_file) {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!("writeback: failed to strip update block: {}", error);
                    false
                }
            };

            let error_block_written = if block_stripped {
                match write_error_block_messages(agent_file, &teaching_messages) {
                    Ok(()) => true,
                    Err(error) => {
                        tracing::warn!("writeback: failed to write error block: {}", error);
                        false
                    }
                }
            } else {
                false
            };

            return Ok(WritebackOutcome {
                block_found: true,
                apply_results: vec![],
                parse_errors: merge_parse_errors(parse_result.errors, task_parse_result.errors),
                block_stripped,
                error_block_written,
            });
        }
    };

    let codebase_name = codebase.name.0.clone();

    // 4. Apply task snapshot first, then let explicit update commands override it.
    let task_snapshot_changed = applier::reconcile_task_snapshot(&mut codebase, &task_parse_result.tasks);
    let apply_results = applier::apply(&mut codebase, &parse_result.commands);

    // 5. Save registry atomically.
    let should_save_registry = !apply_results.is_empty() || task_snapshot_changed;
    if should_save_registry {
        registry::save_codebase_at(home, &project_name, &codebase)?;
    }

    if !apply_results.is_empty() {
        apply_hash_store_lifecycle(home, &codebase_name, &parse_result.commands, &apply_results)?;
    }

    // 6. Strip update block atomically.
    let block_stripped = if update_block.is_some() {
        match strip_update_block(agent_file) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("writeback: failed to strip update block: {}", e);
                false
            }
        }
    } else {
        false
    };

    // 7. Inject error block atomically when needed.
    let all_parse_errors = merge_parse_errors(parse_result.errors.clone(), task_parse_result.errors.clone());
    let error_block_written = if !all_parse_errors.is_empty() {
        match write_error_block_messages(
            agent_file,
            &all_parse_errors.iter().map(|error| error.to_string()).collect::<Vec<_>>(),
        ) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("writeback: failed to write error block: {}", e);
                false
            }
        }
    } else {
        false
    };

    // 8. Run full sync pipeline.
    if should_save_registry {
        if let Err(e) = pipeline::run(home, SyncScope::Codebase(codebase_name.clone()), false) {
        tracing::warn!(
            "writeback: sync pipeline failed after apply for {}: {}",
            codebase_name,
            e
        );
        }
    }

    // 9. Count apply errors
    let apply_errors = apply_results
        .iter()
        .filter(|r| matches!(r.outcome, ApplyOutcome::Error { .. }))
        .count();
    let commands_applied = apply_results
        .iter()
        .filter(|r| matches!(r.outcome, ApplyOutcome::Applied))
        .count();
    let command_list = apply_results
        .iter()
        .map(|r| r.command.as_str())
        .collect::<Vec<_>>()
        .join(",");

    // 9. Append event to sync-events log.
    let ts = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let agent_file_str = agent_file.to_string_lossy();
    let event = WritebackEvent {
        timestamp: &ts,
        agent_file: &agent_file_str,
        codebase_name: &codebase_name,
        commands_applied,
        parse_errors: all_parse_errors.len(),
        apply_errors,
        commands: &command_list,
        block_stripped,
        error_block_written,
    };
    if let Err(e) = log_event(home, &event) {
        tracing::warn!("writeback: failed to write event log: {}", e);
    }

    Ok(WritebackOutcome {
        block_found: update_block.is_some() || task_snapshot_changed || !all_parse_errors.is_empty(),
        apply_results,
        parse_errors: all_parse_errors,
        block_stripped,
        error_block_written,
    })
}

fn merge_parse_errors(
    mut left: Vec<self::types::ParseError>,
    right: Vec<self::types::ParseError>,
) -> Vec<self::types::ParseError> {
    left.extend(right);
    left
}

fn apply_hash_store_lifecycle(
    home: &Path,
    codebase_name: &str,
    commands: &[WritebackCommand],
    apply_results: &[self::types::ApplyResult],
) -> Result<(), SyncError> {
    let mut store = hash_store::load_at(home, codebase_name)?;
    let mut changed = false;

    for (command, result) in commands.iter().zip(apply_results.iter()) {
        if !matches!(result.outcome, ApplyOutcome::Applied) {
            continue;
        }

        match command {
            WritebackCommand::FileCreated { path } => {
                let key = path.to_string_lossy().to_string();
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    store.files.entry(key)
                {
                    entry.insert(String::new());
                    changed = true;
                }
            }
            WritebackCommand::FileDeleted { path } => {
                let key = path.to_string_lossy().to_string();
                if store.files.remove(&key).is_some() {
                    changed = true;
                }
            }
            _ => {}
        }
    }

    if changed {
        hash_store::save_at(home, codebase_name, &store)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal: codebase lookup
// ---------------------------------------------------------------------------

/// Find which registered codebase owns `agent_file` by checking
/// `AgentKind::output_paths` for each codebase.
fn find_owning_codebase(
    all: &[(orchestra_core::types::ProjectName, orchestra_core::types::Codebase)],
    agent_file: &Path,
    commands: &[WritebackCommand],
) -> Option<(orchestra_core::types::ProjectName, orchestra_core::types::Codebase)> {
    // Canonicalize to resolve symlinks / relative components.
    let canonical_agent = std::fs::canonicalize(agent_file).unwrap_or_else(|_| agent_file.to_path_buf());

    if let Some(hint_name) = commands.iter().rev().find_map(|command| match command {
        WritebackCommand::CodebaseHint { codebase } => Some(codebase.as_str()),
        _ => None,
    }) {
        if let Some((project_name, codebase)) = all
            .iter()
            .find(|(_, codebase)| codebase.name.0 == hint_name)
            .cloned()
        {
            return Some((project_name, codebase));
        }
    }

    for (project_name, codebase) in all {
        let codebase_root = &codebase.path;
        for agent_kind in AgentKind::all() {
            for output_path in agent_kind.output_paths(codebase_root) {
                let canonical_output = std::fs::canonicalize(&output_path)
                    .unwrap_or_else(|_| output_path.clone());
                if canonical_agent == canonical_output {
                    return Some((project_name.clone(), codebase.clone()));
                }
            }
        }
    }
    None
}

/// Collect all managed agent file paths for all registered codebases.
/// Used by the daemon watcher to know which files to watch.
pub fn managed_agent_paths(
    all: &[(orchestra_core::types::ProjectName, orchestra_core::types::Codebase)],
) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    for (_project, codebase) in all {
        for agent_kind in AgentKind::all() {
            for output_path in agent_kind.output_paths(&codebase.path) {
                paths.push(output_path);
            }
        }
        paths.push(guide_path(&codebase.path));
        paths.push(pilot_path(&codebase.path));
    }
    paths
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use orchestra_core::{
        registry,
        types::{CodebaseName, ProjectName, ProjectType, Task, TaskId, TaskStatus},
    };
    use std::fs;
    use tempfile::TempDir;

    fn setup(home: &TempDir, workspace: &TempDir, cb_name: &str) {
        let codebase_dir = workspace.path().join(cb_name);
        fs::create_dir_all(&codebase_dir).expect("mkdir");
        registry::init_at(
            codebase_dir,
            ProjectName::from("copnow"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");
    }

    #[test]
    fn no_block_returns_no_block_outcome() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        setup(&home, &workspace, "test_cb");

        // Run initial sync so agent files exist
        pipeline::run(home.path(), SyncScope::Codebase("test_cb".to_owned()), false)
            .expect("initial sync");

        let agent_file = workspace.path().join("test_cb").join("orchestra/controls/CLAUDE.md");
        assert!(agent_file.exists(), "control CLAUDE.md should exist after sync");

        let outcome = process_writeback(home.path(), &agent_file).expect("process");
        assert!(!outcome.block_found);
    }

    #[test]
    fn process_writeback_convention_applied_and_stripped() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        setup(&home, &workspace, "test_cb");

        pipeline::run(home.path(), SyncScope::Codebase("test_cb".to_owned()), false)
            .expect("initial sync");

        let agent_file = workspace.path().join("test_cb").join("orchestra/controls/CLAUDE.md");

        // Append an update block
        let original = fs::read_to_string(&agent_file).expect("read");
        let with_block = format!(
            "{original}\n<!-- orchestra:update -->\nconvention_added: Always write tests\n<!-- /orchestra:update -->\n"
        );
        fs::write(&agent_file, &with_block).expect("write");

        let outcome = process_writeback(home.path(), &agent_file).expect("process");

        assert!(outcome.block_found);
        assert!(outcome.block_stripped, "block should be stripped");
        assert!(outcome.parse_errors.is_empty(), "no parse errors expected");

        // Update block should be gone from file
        let result = fs::read_to_string(&agent_file).expect("read after");
        assert!(!result.contains("orchestra:update"), "update block stripped");

        // Convention should be in registry
        let all = registry::list_codebases_at(home.path()).expect("list");
        let (_, cb) = all.into_iter().find(|(_, cb)| cb.name.0 == "test_cb").expect("codebase");
        assert_eq!(cb.conventions, vec!["Always write tests"]);
    }

    #[test]
    fn process_writeback_invalid_command_writes_error_block() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        setup(&home, &workspace, "test_cb");

        pipeline::run(home.path(), SyncScope::Codebase("test_cb".to_owned()), false)
            .expect("initial sync");

        let agent_file = workspace.path().join("test_cb").join("orchestra/controls/CLAUDE.md");

        let original = fs::read_to_string(&agent_file).expect("read");
        let with_block = format!(
            "{original}\n<!-- orchestra:update -->\ntask_done: T-99\n<!-- /orchestra:update -->\n"
        );
        fs::write(&agent_file, with_block).expect("write");

        let outcome = process_writeback(home.path(), &agent_file).expect("process");

        assert!(outcome.error_block_written, "error block should be written");
        assert!(!outcome.parse_errors.is_empty(), "should have parse errors");

        let result = fs::read_to_string(&agent_file).expect("read after");
        assert!(result.contains("orchestra:error"), "error block present in file");
        assert!(!result.contains("orchestra:update"), "update block stripped");
    }

    #[test]
    fn process_writeback_typo_command_includes_suggestion() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        setup(&home, &workspace, "test_cb");

        pipeline::run(home.path(), SyncScope::Codebase("test_cb".to_owned()), false)
            .expect("initial sync");

        let agent_file = workspace.path().join("test_cb").join("orchestra/controls/CLAUDE.md");
        let original = fs::read_to_string(&agent_file).expect("read");
        let with_block = format!(
            "{original}\n<!-- orchestra:update -->\ntak_completed: T-42\n<!-- /orchestra:update -->\n"
        );
        fs::write(&agent_file, with_block).expect("write");

        let outcome = process_writeback(home.path(), &agent_file).expect("process");

        assert!(outcome.block_found);
        assert!(outcome.block_stripped);
        assert!(outcome.error_block_written);

        let result = fs::read_to_string(&agent_file).expect("read after");
        assert!(result.contains("orchestra:error"));
        assert!(result.contains("tak_completed"));
        assert!(result.contains("task_completed"));
        assert!(!result.contains("orchestra:update"));
    }

    #[test]
    fn process_writeback_task_completed_end_to_end() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let cb_dir = workspace.path().join("test_cb");
        fs::create_dir_all(&cb_dir).expect("mkdir");

        // Init with a task
        let now = Utc::now();
        let task = Task {
            id: TaskId::from("T-42"),
            title: "Implement writeback".to_owned(),
            status: TaskStatus::Pending,
            description: None,
            subtasks: vec![],
            notes: vec![],
            created_at: now,
            updated_at: now,
        };
        let project = orchestra_core::types::Project {
            name: orchestra_core::types::ProjectName::from("test_cb"),
            project_type: ProjectType::Backend,
            tasks: vec![task],
            agents: vec![],
        };
        let codebase = orchestra_core::types::Codebase {
            name: CodebaseName::from("test_cb"),
            path: cb_dir.clone(),
            projects: vec![project],
            conventions: vec![],
            skills: vec![],
            notes: vec![],
            tracked_files: vec![],
            created_at: now,
            updated_at: now,
        };
        registry::save_codebase_at(home.path(), &ProjectName::from("copnow"), &codebase)
            .expect("save");

        // Create the project directory first
        registry::project_dir_at(home.path(), &ProjectName::from("copnow")).expect("project dir");
        registry::save_codebase_at(home.path(), &ProjectName::from("copnow"), &codebase)
            .expect("save again");

        pipeline::run(home.path(), SyncScope::Codebase("test_cb".to_owned()), false)
            .expect("initial sync");

        let agent_file = cb_dir.join("orchestra/controls/CLAUDE.md");
        let original = fs::read_to_string(&agent_file).expect("read");
        let with_block = format!(
            "{original}\n<!-- orchestra:update -->\ntask_completed: T-42\n<!-- /orchestra:update -->\n"
        );
        fs::write(&agent_file, with_block).expect("write");

        let outcome = process_writeback(home.path(), &agent_file).expect("process");

        assert!(outcome.block_found);
        assert!(outcome.block_stripped);

        // Check task is Done in registry
        let all = registry::list_codebases_at(home.path()).expect("list");
        let (_, cb) = all.into_iter().find(|(_, cb)| cb.name.0 == "test_cb").expect("cb");
        let task = &cb.projects[0].tasks[0];
        assert_eq!(task.status, TaskStatus::Done, "task should be marked Done");
    }

    #[test]
    fn process_writeback_task_block_updates_registry_and_syncs_other_files() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let cb_dir = workspace.path().join("task_block_cb");
        fs::create_dir_all(&cb_dir).expect("mkdir");

        registry::init_at(
            cb_dir.clone(),
            ProjectName::from("copnow"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");

        pipeline::run(home.path(), SyncScope::Codebase("task_block_cb".to_owned()), false)
            .expect("initial sync");

        let claude_file = cb_dir.join("orchestra/controls/CLAUDE.md");
        let original = fs::read_to_string(&claude_file).expect("read claude");
        let updated = original.replace(
            "<!-- Add rows like: | T-001 | Example task | pending | optional description | -->",
            "| T-123 | Unify tasks | in_progress | added from CLAUDE |",
        );
        fs::write(&claude_file, updated).expect("write claude");

        let outcome = process_writeback(home.path(), &claude_file).expect("process");
        assert!(outcome.block_found);
        assert!(outcome.parse_errors.is_empty());

        let all = registry::list_codebases_at(home.path()).expect("list");
        let (_, cb) = all
            .into_iter()
            .find(|(_, cb)| cb.name.0 == "task_block_cb")
            .expect("cb");
        assert_eq!(cb.projects[0].tasks[0].id.0, "T-123");
        assert_eq!(cb.projects[0].tasks[0].status, TaskStatus::InProgress);

        let agents = fs::read_to_string(cb_dir.join("orchestra/controls/AGENTS.md")).expect("read agents");
        assert!(agents.contains("T-123"));
        assert!(agents.contains("Unify tasks"));
    }

    #[test]
    fn process_writeback_ignores_block_outside_edge_windows() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        setup(&home, &workspace, "test_cb");

        pipeline::run(home.path(), SyncScope::Codebase("test_cb".to_owned()), false)
            .expect("initial sync");

        let agent_file = workspace.path().join("test_cb").join("orchestra/controls/CLAUDE.md");
        let mut content = String::new();
        for _ in 0..30 {
            content.push_str("line\n");
        }
        content.push_str("<!-- orchestra:update -->\n");
        content.push_str("convention_added: middle block\n");
        content.push_str("<!-- /orchestra:update -->\n");
        for _ in 0..30 {
            content.push_str("line\n");
        }
        fs::write(&agent_file, &content).expect("write");

        let outcome = process_writeback(home.path(), &agent_file).expect("process");
        assert!(!outcome.block_found, "block should be ignored by position constraint");

        let after = fs::read_to_string(&agent_file).expect("read after");
        assert!(after.contains("orchestra:update"), "ignored block should remain untouched");
    }

    #[test]
    fn process_writeback_unmapped_file_writes_teaching_error_block() {
        let home = TempDir::new().unwrap();
        let file = home.path().join("unmanaged.md");
        let content = [
            "Header",
            "<!-- orchestra:update -->",
            "task_completed: T-1",
            "<!-- /orchestra:update -->",
        ]
        .join("\n");
        fs::write(&file, content).expect("write unmanaged file");

        let outcome = process_writeback(home.path(), &file).expect("process");
        assert!(outcome.block_found);
        assert!(outcome.block_stripped);
        assert!(outcome.error_block_written, "teaching block should be written for unmapped files");

        let updated = fs::read_to_string(&file).expect("read unmanaged file after");
        assert!(updated.contains("orchestra:error"));
        assert!(updated.contains("File not associated with any registered codebase"));
        assert!(!updated.contains("orchestra:update"));
    }

    #[test]
    fn process_writeback_codebase_hint_maps_unmanaged_file() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        setup(&home, &workspace, "test_cb");

        pipeline::run(home.path(), SyncScope::Codebase("test_cb".to_owned()), false)
            .expect("initial sync");

        let file = home.path().join("delegated-output.md");
        let content = [
            "Header",
            "<!-- orchestra:update -->",
            "codebase_hint: test_cb",
            "convention_added: Delegated writeback convention",
            "<!-- /orchestra:update -->",
        ]
        .join("\n");
        fs::write(&file, content).expect("write delegated file");

        let outcome = process_writeback(home.path(), &file).expect("process");
        assert!(outcome.block_found);
        assert!(outcome.block_stripped);
        assert!(!outcome.error_block_written, "hinted file should resolve to codebase");
        assert!(outcome.parse_errors.is_empty());

        let all = registry::list_codebases_at(home.path()).expect("list");
        let (_, cb) = all.into_iter().find(|(_, cb)| cb.name.0 == "test_cb").expect("cb");
        assert!(cb
            .conventions
            .iter()
            .any(|entry| entry == "Delegated writeback convention"));
    }
}
