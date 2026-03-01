//! Orchestra — AI agent file management CLI.
//!
//! # Usage
//!
//! ```text
//! orchestra init <path> --project <name> [--type backend|frontend|mobile|ml] [--detect]
//! orchestra project list
//! orchestra project add <name> [--type ...]
//! orchestra sync <codebase> [--dry-run]
//! orchestra sync --all [--dry-run]
//! orchestra status [--project <name>] [--json]
//! orchestra diff <codebase>
//! orchestra daemon start|stop|status|install|uninstall|logs
//! ```

mod commands;

use std::fmt;
use std::str::FromStr;

use anyhow::Result;
use clap::{Parser, Subcommand};

use commands::{
    daemon::DaemonCommand, diff::DiffArgs, init::InitArgs, project::ProjectCommand,
    status::StatusArgs, sync::SyncArgs,
};
use orchestra_core::types::ProjectType;

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "orchestra",
    version,
    about = "Manage AI coding agent files across multiple codebases",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a codebase in the Orchestra registry.
    Init(InitArgs),

    /// Manage projects within the active registry.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },

    /// Render and write per-agent instruction files for a codebase.
    Sync(SyncArgs),

    /// Show staleness status across registered codebases.
    Status(StatusArgs),

    /// Show unified diff of what sync would write for a codebase.
    Diff(DiffArgs),

    /// Manage Orchestra background daemon and launchd integration.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

// ---------------------------------------------------------------------------
// Shared ProjectType argument — parsed from CLI strings, converts to core type
// ---------------------------------------------------------------------------

/// Thin wrapper so clap can parse `ProjectType` from CLI args.
#[derive(Debug, Clone, Default)]
pub struct ProjectTypeArg(pub ProjectType);

impl FromStr for ProjectTypeArg {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "backend" => Ok(Self(ProjectType::Backend)),
            "frontend" => Ok(Self(ProjectType::Frontend)),
            "mobile" => Ok(Self(ProjectType::Mobile)),
            "ml" => Ok(Self(ProjectType::Ml)),
            other => Err(format!(
                "unknown project type '{other}'; expected: backend, frontend, mobile, ml"
            )),
        }
    }
}

impl fmt::Display for ProjectTypeArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<ProjectTypeArg> for ProjectType {
    fn from(p: ProjectTypeArg) -> Self {
        p.0
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init(args) => args.run(),
        Commands::Project { command } => commands::project::run(command),
        Commands::Sync(args) => args.run(),
        Commands::Status(args) => args.run(),
        Commands::Diff(args) => args.run(),
        Commands::Daemon { command } => commands::daemon::run(command),
    }
}
