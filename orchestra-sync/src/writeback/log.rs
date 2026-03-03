//! Event logger — appends sync-events to `~/.orchestra/logs/sync-events.log`.
//!
//! Each event is a single JSON line (NDJSON format) for easy machine parsing
//! and grep-based inspection.

use std::io::Write as _;
use std::path::Path;

use crate::error::{io_err, SyncError};

/// A loggable event from the writeback pipeline.
#[derive(Debug)]
pub struct WritebackEvent<'a> {
    /// ISO8601 timestamp of the event.
    pub timestamp: &'a str,
    /// Absolute path of the agent file that contained the update block.
    pub agent_file: &'a str,
    /// Name of the codebase that was updated.
    pub codebase_name: &'a str,
    /// Number of commands successfully applied.
    pub commands_applied: usize,
    /// Number of parse errors.
    pub parse_errors: usize,
    /// Number of apply errors.
    pub apply_errors: usize,
    /// Command labels applied/attempted in this writeback cycle.
    pub commands: &'a str,
    /// Whether the update block was stripped.
    pub block_stripped: bool,
    /// Whether an error block was written.
    pub error_block_written: bool,
}

/// Append a single NDJSON line to `<home>/.orchestra/logs/sync-events.log`.
///
/// Creates the log file (and parent directory) if absent.
pub fn log_event(home: &Path, event: &WritebackEvent<'_>) -> Result<(), SyncError> {
    let log_path = home
        .join(".orchestra")
        .join("logs")
        .join("sync-events.log");

    // Ensure logs directory exists.
    if let Some(parent) = log_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
        }
    }

    let line = format!(
        "{{\"timestamp\":\"{}\",\"agent_file\":\"{}\",\"codebase\":\"{}\",\"commands_applied\":{},\"parse_errors\":{},\"apply_errors\":{},\"commands\":\"{}\",\"block_stripped\":{},\"error_block_written\":{}}}\n",
        event.timestamp,
        event.agent_file,
        event.codebase_name,
        event.commands_applied,
        event.parse_errors,
        event.apply_errors,
        event.commands,
        event.block_stripped,
        event.error_block_written,
    );

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| io_err(&log_path, e))?;

    file.write_all(line.as_bytes()).map_err(|e| io_err(&log_path, e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn log_event_creates_file_if_absent() {
        let home = TempDir::new().unwrap();
        let event = WritebackEvent {
            timestamp: "2026-03-01T22:00:00Z",
            agent_file: "/tmp/test_cb/CLAUDE.md",
            codebase_name: "test_cb",
            commands_applied: 1,
            parse_errors: 0,
            apply_errors: 0,
            commands: "task_completed|T-42",
            block_stripped: true,
            error_block_written: false,
        };

        log_event(home.path(), &event).expect("log_event");

        let log_path = home.path().join(".orchestra").join("logs").join("sync-events.log");
        assert!(log_path.exists(), "log file should be created");
    }

    #[test]
    fn log_event_appends_json_line() {
        let home = TempDir::new().unwrap();

        for i in 0..3u32 {
            let ts = format!("2026-03-01T22:00:0{i}Z");
            let event = WritebackEvent {
                timestamp: &ts,
                agent_file: "/tmp/cb/CLAUDE.md",
                codebase_name: "cb",
                commands_applied: i as usize,
                parse_errors: 0,
                apply_errors: 0,
                commands: "task_completed|T-42",
                block_stripped: true,
                error_block_written: false,
            };
            log_event(home.path(), &event).expect("log_event");
        }

        let log_path = home.path().join(".orchestra").join("logs").join("sync-events.log");
        let contents = std::fs::read_to_string(log_path).unwrap();
        let lines: Vec<_> = contents.lines().collect();
        assert_eq!(lines.len(), 3, "three events appended as three lines");

        // Each line should be valid JSON containing expected fields.
        for line in &lines {
            assert!(line.starts_with('{'), "each line is a JSON object");
            assert!(line.contains("\"codebase\":\"cb\""));
            assert!(line.contains("\"agent_file\":\"/tmp/cb/CLAUDE.md\""));
        }
    }

    #[test]
    fn log_event_records_error_fields() {
        let home = TempDir::new().unwrap();
        let event = WritebackEvent {
            timestamp: "2026-03-01T22:00:00Z",
            agent_file: "/tmp/cb/CLAUDE.md",
            codebase_name: "cb",
            commands_applied: 0,
            parse_errors: 2,
            apply_errors: 1,
            commands: "task_completed|T-42",
            block_stripped: true,
            error_block_written: true,
        };

        log_event(home.path(), &event).expect("log_event");

        let log_path = home.path().join(".orchestra").join("logs").join("sync-events.log");
        let contents = std::fs::read_to_string(log_path).unwrap();
        assert!(contents.contains("\"parse_errors\":2"));
        assert!(contents.contains("\"apply_errors\":1"));
        assert!(contents.contains("\"error_block_written\":true"));
    }
}
