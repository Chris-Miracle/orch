//! Shared types for the writeback subsystem.

use std::path::PathBuf;

use orchestra_core::types::TaskStatus;

// ---------------------------------------------------------------------------
// Command enum
// ---------------------------------------------------------------------------

/// A parsed instruction from an agent-written `<!-- orchestra:update -->` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WritebackCommand {
    /// Hint for ownership mapping when writeback originates from delegated/worktree context.
    CodebaseHint { codebase: String },
    /// Mark a task as done by its ID.
    TaskCompleted { task_id: String },
    /// Mark a task as in-progress by ID.
    TaskStarted { task_id: String },
    /// Mark a task as blocked with reason.
    TaskBlocked { task_id: String, reason: String },
    /// Mark a subtask under task as done.
    SubtaskDone {
        task_id: String,
        subtask_title: String,
    },
    /// Add a convention to the codebase (deduplicated on apply).
    ConventionAdded { text: String },
    /// Record a discovered skill on the codebase.
    SkillDiscovered { id: String, description: String },
    /// Append a timestamped note to the codebase's notes list.
    Note { text: String },
    /// Records use of a subagent (log-only).
    SubagentUsed { id: String },
    /// Registers a created tracked file path.
    FileCreated { path: PathBuf },
    /// Registers a deleted tracked file path.
    FileDeleted { path: PathBuf },
}

/// A parsed task row from the canonical Orchestra task block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub task_id: String,
    pub title: String,
    pub status: TaskStatus,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Parse types
// ---------------------------------------------------------------------------

/// A single parse error for one malformed line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-indexed line number within the block body (not the file).
    pub line_number: usize,
    /// The raw line content.
    pub raw_line: String,
    /// Human-readable error message with correction syntax.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {}: {} — {}",
            self.line_number,
            self.raw_line.trim(),
            self.message
        )
    }
}

/// Result of parsing a complete update block.
#[derive(Debug)]
pub struct ParseResult {
    pub commands: Vec<WritebackCommand>,
    pub errors: Vec<ParseError>,
}

/// Result of parsing the canonical task block.
#[derive(Debug)]
pub struct TaskParseResult {
    pub tasks: Vec<TaskSnapshot>,
    pub errors: Vec<ParseError>,
}

// ---------------------------------------------------------------------------
// Apply result
// ---------------------------------------------------------------------------

/// The outcome of applying a single writeback command to registry state.
#[derive(Debug, Clone)]
pub struct ApplyResult {
    pub command: String,
    pub outcome: ApplyOutcome,
}

/// The individual outcome of a command application.
#[derive(Debug, Clone)]
pub enum ApplyOutcome {
    Applied,
    Skipped { reason: String },
    Error { message: String },
}

// ---------------------------------------------------------------------------
// Writeback outcome (top-level)
// ---------------------------------------------------------------------------

/// Summary returned by `process_writeback` after a full writeback cycle.
#[derive(Debug)]
pub struct WritebackOutcome {
    /// Whether an update block was found in the file.
    pub block_found: bool,
    /// Results from applying each command.
    pub apply_results: Vec<ApplyResult>,
    /// Parse errors encountered.
    pub parse_errors: Vec<ParseError>,
    /// Whether the update block was successfully stripped.
    pub block_stripped: bool,
    /// Whether an error block was written back.
    pub error_block_written: bool,
}

impl WritebackOutcome {
    pub fn no_block() -> Self {
        Self {
            block_found: false,
            apply_results: vec![],
            parse_errors: vec![],
            block_stripped: false,
            error_block_written: false,
        }
    }
}
