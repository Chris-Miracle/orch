//! `orchestra diff <codebase>` — show unified diffs for what sync would write.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use orchestra_sync::diff_codebase;

/// Arguments for `orchestra diff`.
#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Codebase name to diff.
    pub codebase: String,
}

impl DiffArgs {
    pub fn run(self) -> Result<()> {
        let home: PathBuf = dirs::home_dir().context("could not determine home directory")?;

        let result = diff_codebase(&self.codebase, &home)
            .with_context(|| format!("diff failed for '{}'", self.codebase))?;

        if result.diffs.is_empty() {
            println!("No differences for '{}'.", result.codebase_name);
            return Ok(());
        }

        for diff in result.diffs {
            print!("{}", diff.unified_diff);
            if !diff.unified_diff.ends_with('\n') {
                println!();
            }
        }

        Ok(())
    }
}
