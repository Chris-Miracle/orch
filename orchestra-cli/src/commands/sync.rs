//! `orchestra sync` — render and write per-agent files for a codebase.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use orchestra_sync::{
    pipeline::{self, SyncScope},
    WriteResult,
};

/// Arguments for `orchestra sync`.
#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Name of the codebase to sync (omit when using `--all`).
    pub codebase: Option<String>,

    /// Sync every registered codebase.
    #[arg(long, conflicts_with = "codebase")]
    pub all: bool,

    /// Show what would be written without actually writing any files.
    #[arg(long)]
    pub dry_run: bool,
}

impl SyncArgs {
    pub fn run(self) -> Result<()> {
        let home: PathBuf = dirs::home_dir().context("could not determine home directory")?;

        if self.all {
            let results =
                pipeline::run(&home, SyncScope::All, self.dry_run).context("sync --all failed")?;
            for r in &results {
                print_results(&r.codebase_name, &r.writes, self.dry_run);
            }
            if results.is_empty() {
                println!("No codebases registered. Run `orchestra init` first.");
            }
        } else {
            let name = self
                .codebase
                .clone()
                .context("provide a codebase name or use --all")?;
            let mut results = pipeline::run(&home, SyncScope::Codebase(name.clone()), self.dry_run)
                .with_context(|| format!("sync failed for '{name}'"))?;
            if let Some(result) = results.pop() {
                print_results(&result.codebase_name, &result.writes, self.dry_run);
            }
        }

        Ok(())
    }
}

fn print_results(codebase_name: &str, writes: &[WriteResult], dry_run: bool) {
    let prefix = if dry_run { "[dry-run] " } else { "" };
    let written: Vec<_> = writes
        .iter()
        .filter(|r| {
            matches!(
                r,
                WriteResult::Written { .. } | WriteResult::WouldWrite { .. }
            )
        })
        .collect();
    let unchanged: Vec<_> = writes
        .iter()
        .filter(|r| matches!(r, WriteResult::Unchanged { .. }))
        .collect();

    if written.is_empty() && unchanged.is_empty() {
        println!("{prefix}✓ '{codebase_name}' — nothing to do");
        return;
    }

    println!(
        "{prefix}✓ '{codebase_name}' synced ({} written, {} unchanged)",
        written.len(),
        unchanged.len()
    );

    for r in writes {
        match r {
            WriteResult::Written { path } => println!("  ✎  {}", path.display()),
            WriteResult::WouldWrite { path } => println!("  ~  {}", path.display()),
            WriteResult::Unchanged { path } => println!("  ·  {}", path.display()),
        }
    }
}
