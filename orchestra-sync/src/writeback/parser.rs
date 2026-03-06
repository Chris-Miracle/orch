//! Writeback parser — detects and parses `<!-- orchestra:update -->` blocks.
//!
//! FRD grammar:
//! ```text
//! task_completed: <task-id>
//! task_started: <task-id>
//! task_blocked: <task-id> | <reason>
//! subtask_done: <task-id>/<subtask-title>
//! skill_discovered: <skill-id> | <description>
//! convention_added: <text>
//! note: <text>
//! subagent_used: <subagent-id>
//! file_created: <relative/path>
//! file_deleted: <relative/path>
//! ```

use std::path::PathBuf;

use orchestra_core::types::TaskStatus;

use crate::writeback::types::{ParseError, ParseResult, TaskParseResult, TaskSnapshot, WritebackCommand};

const UPDATE_OPEN: &str = "<!-- orchestra:update -->";
const UPDATE_CLOSE: &str = "<!-- /orchestra:update -->";
const TASKS_OPEN: &str = "<!-- orchestra:tasks -->";
const TASKS_CLOSE: &str = "<!-- /orchestra:tasks -->";
const EDGE_LINE_WINDOW: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateBlock<'a> {
    pub content: &'a str,
    pub start_line: usize,
    pub end_line: usize,
}

impl<'a> UpdateBlock<'a> {
    pub fn in_allowed_window(&self, total_lines: usize) -> bool {
        if self.start_line <= EDGE_LINE_WINDOW {
            return true;
        }
        let lower_bound = total_lines.saturating_sub(EDGE_LINE_WINDOW).saturating_add(1);
        self.end_line >= lower_bound
    }
}

pub fn has_update_block(content: &str) -> bool {
    find_update_block(content).is_some()
}

pub fn has_task_block(content: &str) -> bool {
    find_task_block(content).is_some()
}

pub fn find_update_block(content: &str) -> Option<UpdateBlock<'_>> {
    let start = content.find(UPDATE_OPEN)?;
    let after_open = start + UPDATE_OPEN.len();
    let close_rel = content[after_open..].find(UPDATE_CLOSE)?;
    let end = after_open + close_rel;

    let start_line = content[..start].bytes().filter(|b| *b == b'\n').count() + 1;
    let end_line = content[..end].bytes().filter(|b| *b == b'\n').count() + 1;

    Some(UpdateBlock {
        content: &content[after_open..end],
        start_line,
        end_line,
    })
}

pub fn find_task_block(content: &str) -> Option<UpdateBlock<'_>> {
    let start = content.find(TASKS_OPEN)?;
    let after_open = start + TASKS_OPEN.len();
    let close_rel = content[after_open..].find(TASKS_CLOSE)?;
    let end = after_open + close_rel;

    let start_line = content[..start].bytes().filter(|b| *b == b'\n').count() + 1;
    let end_line = content[..end].bytes().filter(|b| *b == b'\n').count() + 1;

    Some(UpdateBlock {
        content: &content[after_open..end],
        start_line,
        end_line,
    })
}

pub fn parse_block(block_content: &str) -> ParseResult {
    let mut commands = Vec::new();
    let mut errors = Vec::new();

    for (line_idx, raw_line) in block_content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match parse_line(line) {
            Ok(cmd) => commands.push(cmd),
            Err(message) => errors.push(ParseError {
                line_number: line_idx + 1,
                raw_line: raw_line.to_owned(),
                message,
            }),
        }
    }

    ParseResult { commands, errors }
}

pub fn parse_task_block(block_content: &str) -> TaskParseResult {
    let mut tasks = Vec::new();
    let mut errors = Vec::new();
    let mut seen_ids = std::collections::BTreeSet::new();

    for (line_idx, raw_line) in block_content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("<!--")
            || line.starts_with("|---")
            || line.eq_ignore_ascii_case("| id | title | status | description |")
            || !line.starts_with('|')
        {
            continue;
        }

        match parse_task_line(line) {
            Ok(task) => {
                if !seen_ids.insert(task.task_id.clone()) {
                    errors.push(ParseError {
                        line_number: line_idx + 1,
                        raw_line: raw_line.to_owned(),
                        message: format!("duplicate task id '{}' in orchestra:tasks block", task.task_id),
                    });
                    continue;
                }
                tasks.push(task);
            }
            Err(message) => errors.push(ParseError {
                line_number: line_idx + 1,
                raw_line: raw_line.to_owned(),
                message,
            }),
        }
    }

    TaskParseResult { tasks, errors }
}

fn parse_task_line(line: &str) -> Result<TaskSnapshot, String> {
    let cells: Vec<String> = line
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect();

    if cells.len() < 3 {
        return Err("task rows require at least '| ID | Title | Status |'".to_owned());
    }

    let task_id = cells[0].trim();
    let title = cells[1].trim();
    let status_raw = cells[2].trim();
    let description = cells
        .get(3)
        .map(|cell| cell.trim())
        .filter(|cell| !cell.is_empty() && *cell != "-" && !cell.eq_ignore_ascii_case("none"))
        .map(|cell| cell.to_string());

    if task_id.is_empty() {
        return Err("task id must not be empty".to_owned());
    }
    if title.is_empty() {
        return Err("task title must not be empty".to_owned());
    }

    let status = parse_task_status(status_raw)?;
    Ok(TaskSnapshot {
        task_id: task_id.to_owned(),
        title: title.to_owned(),
        status,
        description,
    })
}

fn parse_task_status(value: &str) -> Result<TaskStatus, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pending" | "todo" | "open" => Ok(TaskStatus::Pending),
        "in_progress" | "in-progress" | "in progress" | "active" | "doing" => {
            Ok(TaskStatus::InProgress)
        }
        "blocked" => Ok(TaskStatus::Blocked),
        "done" | "complete" | "completed" => Ok(TaskStatus::Done),
        other => Err(format!(
            "unknown task status '{other}'. Supported: pending, in_progress, blocked, done"
        )),
    }
}

fn parse_line(line: &str) -> Result<WritebackCommand, String> {
    let (verb_raw, value_raw) = line
        .split_once(':')
        .ok_or_else(|| format!("invalid command format: '{line}'. Expected '<command>: <value>'"))?;

    let verb = verb_raw.trim();
    let value = value_raw.trim();

    if value.is_empty() {
        return Err(format!("{verb}: value must not be empty"));
    }

    match verb {
        "codebase_hint" => Ok(WritebackCommand::CodebaseHint {
            codebase: value.to_owned(),
        }),
        "task_completed" => Ok(WritebackCommand::TaskCompleted {
            task_id: value.to_owned(),
        }),
        "task_started" => Ok(WritebackCommand::TaskStarted {
            task_id: value.to_owned(),
        }),
        "task_blocked" => {
            let (task_id_raw, reason_raw) = value.split_once('|').ok_or_else(|| {
                "task_blocked requires '<task-id> | <reason>'".to_owned()
            })?;
            let task_id = task_id_raw.trim();
            let reason = reason_raw.trim();
            if task_id.is_empty() {
                return Err("task_blocked task id must not be empty".to_owned());
            }
            if reason.is_empty() {
                return Err("task_blocked reason must not be empty".to_owned());
            }
            Ok(WritebackCommand::TaskBlocked {
                task_id: task_id.to_owned(),
                reason: reason.to_owned(),
            })
        }
        "subtask_done" => {
            let (task_id_raw, subtask_raw) = value.split_once('/').ok_or_else(|| {
                "subtask_done requires '<task-id>/<subtask-title>'".to_owned()
            })?;
            let task_id = task_id_raw.trim();
            let subtask_title = subtask_raw.trim();
            if task_id.is_empty() {
                return Err("subtask_done task id must not be empty".to_owned());
            }
            if subtask_title.is_empty() {
                return Err("subtask_done subtask title must not be empty".to_owned());
            }
            Ok(WritebackCommand::SubtaskDone {
                task_id: task_id.to_owned(),
                subtask_title: subtask_title.to_owned(),
            })
        }
        "skill_discovered" => {
            let (id_raw, description_raw) = value.split_once('|').ok_or_else(|| {
                "skill_discovered requires '<skill-id> | <description>'".to_owned()
            })?;
            let id = id_raw.trim();
            let description = description_raw.trim();
            if id.is_empty() {
                return Err("skill_discovered id must not be empty".to_owned());
            }
            if description.is_empty() {
                return Err("skill_discovered description must not be empty".to_owned());
            }
            Ok(WritebackCommand::SkillDiscovered {
                id: id.to_owned(),
                description: description.to_owned(),
            })
        }
        "convention_added" => Ok(WritebackCommand::ConventionAdded {
            text: value.to_owned(),
        }),
        "note" => Ok(WritebackCommand::Note {
            text: value.to_owned(),
        }),
        "subagent_used" => Ok(WritebackCommand::SubagentUsed {
            id: value.to_owned(),
        }),
        "file_created" => parse_relative_path(value).map(|path| WritebackCommand::FileCreated { path }),
        "file_deleted" => parse_relative_path(value).map(|path| WritebackCommand::FileDeleted { path }),
        other => Err(format!(
            "unknown command '{other}'. Supported: codebase_hint, task_completed, task_started, task_blocked, subtask_done, skill_discovered, convention_added, note, subagent_used, file_created, file_deleted"
        )),
    }
}

fn parse_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.trim());
    if path.is_absolute() {
        return Err("file path must be relative".to_owned());
    }
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("file path must not contain '..'".to_owned());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_update_block_reports_lines() {
        let content = "a\nb\n<!-- orchestra:update -->\ntask_completed: T-1\n<!-- /orchestra:update -->\n";
        let block = find_update_block(content).expect("block present");
        assert_eq!(block.start_line, 3);
        assert!(block.end_line >= block.start_line);
    }

    #[test]
    fn block_position_constraint_honors_top_and_bottom_windows() {
        let top = "<!-- orchestra:update -->\nnote: hi\n<!-- /orchestra:update -->\nrest\n";
        let top_block = find_update_block(top).expect("top block");
        assert!(top_block.in_allowed_window(top.lines().count()));

        let mut middle = String::new();
        for _ in 0..30 {
            middle.push_str("line\n");
        }
        middle.push_str("<!-- orchestra:update -->\nnote: hi\n<!-- /orchestra:update -->\n");
        for _ in 0..30 {
            middle.push_str("line\n");
        }
        let middle_block = find_update_block(&middle).expect("middle block");
        assert!(!middle_block.in_allowed_window(middle.lines().count()));

        let mut bottom = String::new();
        for _ in 0..25 {
            bottom.push_str("line\n");
        }
        bottom.push_str("<!-- orchestra:update -->\nnote: hi\n<!-- /orchestra:update -->\n");
        let bottom_block = find_update_block(&bottom).expect("bottom block");
        assert!(bottom_block.in_allowed_window(bottom.lines().count()));
    }

    #[test]
    fn parse_block_mixed_valid_and_invalid_aggregates() {
        let block = [
            "task_started: T-1",
            "task_blocked: T-2 | waiting",
            "unknown: whatever",
            "file_created: ../bad",
            "note: keep going",
        ]
        .join("\n");

        let result = parse_block(&block);
        assert_eq!(result.commands.len(), 3);
        assert_eq!(result.errors.len(), 2);
        assert!(result
            .errors
            .iter()
            .any(|e| e.raw_line.contains("unknown: whatever")));
        assert!(result
            .errors
            .iter()
            .any(|e| e.raw_line.contains("file_created: ../bad")));
    }

    #[test]
    fn parses_all_ten_commands() {
        let block = [
            "codebase_hint: copnow_api",
            "task_completed: T-1",
            "task_started: T-2",
            "task_blocked: T-3 | blocked on CI",
            "subtask_done: T-3/Add tests",
            "skill_discovered: rust-async | async coordination",
            "convention_added: Always run tests",
            "note: captured context",
            "subagent_used: qa-reliability-reviewer",
            "file_created: docs/new.md",
            "file_deleted: docs/old.md",
        ]
        .join("\n");

        let result = parse_block(&block);
        assert_eq!(result.commands.len(), 11);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn subtask_done_requires_slash_separator() {
        let result = parse_block("subtask_done: T-1 | Wrong");
        assert_eq!(result.commands.len(), 0);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("<task-id>/<subtask-title>"));
    }

    #[test]
    fn parse_task_block_reads_table_rows() {
        let block = [
            "| ID | Title | Status | Description |",
            "|---|---|---|---|",
            "| T-1 | Ship onboarding | pending | polish prompt |",
            "| T-2 | Review writeback | in_progress | - |",
        ]
        .join("\n");

        let result = parse_task_block(&block);
        assert!(result.errors.is_empty());
        assert_eq!(result.tasks.len(), 2);
        assert_eq!(result.tasks[0].task_id, "T-1");
        assert_eq!(result.tasks[1].status, TaskStatus::InProgress);
        assert_eq!(result.tasks[0].description.as_deref(), Some("polish prompt"));
    }

    #[test]
    fn parse_task_block_rejects_duplicate_ids() {
        let block = [
            "| ID | Title | Status | Description |",
            "|---|---|---|---|",
            "| T-1 | First | pending | - |",
            "| T-1 | Duplicate | blocked | - |",
        ]
        .join("\n");

        let result = parse_task_block(&block);
        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("duplicate task id"));
    }
}
