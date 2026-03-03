//! Block strip and error-block writer for the writeback subsystem.
//!
//! After processing an update block the raw block must be removed from the
//! agent file. If parse errors occurred an `<!-- orchestra:error -->` block
//! is inserted first so the agent can read the correction syntax.

use std::path::Path;

use crate::error::{io_err, SyncError};
use crate::writeback::types::ParseError;

const UPDATE_OPEN: &str = "<!-- orchestra:update -->";
const UPDATE_CLOSE: &str = "<!-- /orchestra:update -->";
const ERROR_OPEN: &str = "<!-- orchestra:error -->";
const ERROR_CLOSE: &str = "<!-- /orchestra:error -->";

// ---------------------------------------------------------------------------
// Error block
// ---------------------------------------------------------------------------

/// Insert an `<!-- orchestra:error -->` block immediately before the update
/// block so the agent can see what went wrong.
///
/// If any existing error block is already present it is replaced.
pub fn write_error_block(path: &Path, errors: &[ParseError]) -> Result<(), SyncError> {
    let content = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;

    // Remove any prior error block.
    let content = remove_error_block(&content);

    // Build the new error block.
    let mut block = String::from("\n");
    block.push_str(ERROR_OPEN);
    block.push('\n');
    block.push_str("The following commands in the update block could not be parsed:\n");
    for e in errors {
        block.push_str(&format!("  {e}\n"));
    }
    block.push_str("Supported commands:\n");
    block.push_str("  task_completed: <task-id>\n");
    block.push_str("  task_started: <task-id>\n");
    block.push_str("  task_blocked: <task-id> | <reason>\n");
    block.push_str("  subtask_done: <task-id>/<subtask-title>\n");
    block.push_str("  skill_discovered: <skill-id> | <description>\n");
    block.push_str("  convention_added: <text>\n");
    block.push_str("  note: <text>\n");
    block.push_str("  subagent_used: <subagent-id>\n");
    block.push_str("  file_created: <relative/path>\n");
    block.push_str("  file_deleted: <relative/path>\n");
    block.push_str(ERROR_CLOSE);
    block.push('\n');

    // Insert the error block before the update block.
    let insertion_point = content.find(UPDATE_OPEN).unwrap_or(content.len());
    let mut new_content = String::with_capacity(content.len() + block.len());
    new_content.push_str(&content[..insertion_point]);
    new_content.push_str(&block);
    new_content.push_str(&content[insertion_point..]);
    let new_content = normalize_blank_lines(&new_content);

    atomic_write_str(path, &new_content)
}

/// Insert an error block with arbitrary free-form messages.
pub fn write_error_block_messages(path: &Path, messages: &[String]) -> Result<(), SyncError> {
    let content = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;

    let content = remove_error_block(&content);

    let mut block = String::from("\n");
    block.push_str(ERROR_OPEN);
    block.push('\n');
    for message in messages {
        block.push_str("- ");
        block.push_str(message);
        block.push('\n');
    }
    block.push_str(ERROR_CLOSE);
    block.push('\n');

    let insertion_point = content.find(UPDATE_OPEN).unwrap_or(content.len());
    let mut new_content = String::with_capacity(content.len() + block.len());
    new_content.push_str(&content[..insertion_point]);
    new_content.push_str(&block);
    new_content.push_str(&content[insertion_point..]);
    let new_content = normalize_blank_lines(&new_content);

    atomic_write_str(path, &new_content)
}

// ---------------------------------------------------------------------------
// Update block strip
// ---------------------------------------------------------------------------

/// Atomically remove the `<!-- orchestra:update --> … <!-- /orchestra:update -->`
/// block from `path`, collapsing excess blank lines at the join point.
pub fn strip_update_block(path: &Path) -> Result<(), SyncError> {
    let content = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let stripped = do_strip_update_block(&content);
    atomic_write_str(path, &stripped)
}

/// Pure strip logic (no I/O) — exposed for testing.
pub(crate) fn do_strip_update_block(content: &str) -> String {
    let Some(start) = content.find(UPDATE_OPEN) else {
        return content.to_owned();
    };

    let after_open = start + UPDATE_OPEN.len();
    let Some(close_rel) = content[after_open..].find(UPDATE_CLOSE) else {
        return content.to_owned();
    };
    let end = after_open + close_rel + UPDATE_CLOSE.len();

    let before = &content[..start];
    let after = &content[end..];

    // Trim trailing whitespace/newlines from `before` and leading from `after`,
    // then join with exactly one blank separator if both sides are non-empty.
    let before_trimmed = before.trim_end_matches(['\n', '\r']);
    let after_trimmed = after.trim_start_matches(['\n', '\r']);

    let joined = match (before_trimmed.is_empty(), after_trimmed.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{after_trimmed}\n"),
        (false, true) => format!("{before_trimmed}\n"),
        (false, false) => format!("{before_trimmed}\n\n{after_trimmed}\n"),
    };

    normalize_blank_lines(&joined)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn remove_error_block(content: &str) -> String {
    let Some(start) = content.find(ERROR_OPEN) else {
        return content.to_owned();
    };
    let after_open = start + ERROR_OPEN.len();
    let Some(close_rel) = content[after_open..].find(ERROR_CLOSE) else {
        return content.to_owned();
    };
    let end = after_open + close_rel + ERROR_CLOSE.len();

    let mut result = String::with_capacity(content.len());
    result.push_str(&content[..start]);
    // consume any trailing newline after the close marker
    let rest = &content[end..];
    result.push_str(rest.trim_start_matches('\n'));
    result
}

fn normalize_blank_lines(content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut prev_blank = false;

    for line in content.lines() {
        let blank = line.trim().is_empty();
        if blank && prev_blank {
            continue;
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        prev_blank = blank;
    }

    if !out.is_empty() {
        out.push('\n');
    }

    out
}

/// Write `content` to `path` atomically via a `.orchestra.tmp` sibling.
fn atomic_write_str(path: &Path, content: &str) -> Result<(), SyncError> {
    let tmp = path.with_extension("orchestra.tmp");
    std::fs::write(&tmp, content).map_err(|e| io_err(&tmp, e))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_err(path, e));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn strip_removes_block_preserves_surrounding() {
        let content = "# Header\n\n<!-- orchestra:update -->\ntask_completed|T-1\n<!-- /orchestra:update -->\n\n## Rest";
        let stripped = do_strip_update_block(content);
        assert!(!stripped.contains("orchestra:update"), "markers should be gone");
        assert!(!stripped.contains("task_completed"), "block body should be gone");
        assert!(stripped.contains("# Header"), "pre-block content preserved");
        assert!(stripped.contains("## Rest"), "post-block content preserved");
    }

    #[test]
    fn strip_no_block_returns_unchanged() {
        let content = "# No block here\nSome text\n";
        assert_eq!(do_strip_update_block(content), content);
    }

    #[test]
    fn strip_only_content_empty_result() {
        let content = "<!-- orchestra:update -->\nnote|something\n<!-- /orchestra:update -->";
        let stripped = do_strip_update_block(content);
        assert!(stripped.trim().is_empty(), "stripping sole block should leave empty content");
    }

    #[test]
    fn strip_collapses_excess_blank_lines() {
        let content = "before\n\n\n<!-- orchestra:update -->\ntask_completed|T-1\n<!-- /orchestra:update -->\n\n\nafter";
        let stripped = do_strip_update_block(content);
        // Should not have 3+ consecutive newlines
        assert!(!stripped.contains("\n\n\n"), "should collapse excess blank lines");
        assert!(stripped.contains("before"));
        assert!(stripped.contains("after"));
    }

    #[test]
    fn strip_atomic_write_roundtrip() {
        let mut file = NamedTempFile::new().unwrap();
        let content = "prefix\n<!-- orchestra:update -->\nnote|hi\n<!-- /orchestra:update -->\nsuffix";
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();

        strip_update_block(file.path()).expect("strip");

        let result = std::fs::read_to_string(file.path()).unwrap();
        assert!(result.contains("prefix"));
        assert!(result.contains("suffix"));
        assert!(!result.contains("orchestra:update"));
    }

    #[test]
    fn write_error_block_inserted_before_update_block() {
        let mut file = NamedTempFile::new().unwrap();
        let content = "Some context\n<!-- orchestra:update -->\ntask_done|T-1\n<!-- /orchestra:update -->";
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();

        let errors = vec![ParseError {
            line_number: 1,
            raw_line: "task_done|T-1".to_owned(),
            message: "unknown command 'task_done'. Supported: task_completed|<id>".to_owned(),
        }];

        write_error_block(file.path(), &errors).expect("write error block");

        let result = std::fs::read_to_string(file.path()).unwrap();
        let error_pos = result.find(ERROR_OPEN).expect("error block present");
        let update_pos = result.find(UPDATE_OPEN).expect("update block present");
        assert!(error_pos < update_pos, "error block should precede update block");
        assert!(result.contains("task_done"), "error line should be mentioned");
        assert!(result.contains("task_completed: <task-id>"), "correction syntax present");
    }

    #[test]
    fn write_error_block_replaces_prior_error_block() {
        let mut file = NamedTempFile::new().unwrap();
        let content = "<!-- orchestra:error -->\nOld error\n<!-- /orchestra:error -->\n<!-- orchestra:update -->\nnote|hi\n<!-- /orchestra:update -->";
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();

        let errors = vec![ParseError {
            line_number: 1,
            raw_line: "bad|line".to_owned(),
            message: "unknown command 'bad'".to_owned(),
        }];

        write_error_block(file.path(), &errors).expect("write error block");

        let result = std::fs::read_to_string(file.path()).unwrap();
        // Only one error block
        assert_eq!(
            result.matches(ERROR_OPEN).count(),
            1,
            "should have exactly one error block"
        );
        assert!(!result.contains("Old error"), "old error should be replaced");
    }
}
