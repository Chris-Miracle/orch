//! `orchestra init <path> --project <name> [--type ...] [--detect]`

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use orchestra_core::{registry, types::ProjectName};

use super::super::ProjectTypeArg;

/// Initialize a codebase in the Orchestra registry.
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Absolute or relative path to the codebase root directory.
    pub path: PathBuf,

    /// Project group name (e.g. "copnow", "myapp"). Creates
    /// ~/.orchestra/projects/<project>/<codebase>.yaml
    #[arg(long, short = 'p')]
    pub project: String,

    /// Project category: backend | frontend | mobile | ml.
    #[arg(long = "type", short = 't', value_name = "TYPE")]
    pub project_type: Option<ProjectTypeArg>,

    /// Auto-detect project type from directory contents (Phase 02+; currently a no-op).
    #[arg(long, conflicts_with = "project_type")]
    pub detect: bool,
}

impl InitArgs {
    pub fn run(self) -> Result<()> {
        let project_type = self.project_type.map(|p| p.into());
        let path = self
            .path
            .canonicalize()
            .with_context(|| format!("cannot resolve path '{}'", self.path.display()))?;

        let project = self.project.clone();
        let codebase = registry::init(path.clone(), ProjectName::from(self.project), project_type)
            .with_context(|| {
                format!(
                    "failed to init '{}' under project '{}'",
                    path.display(),
                    project
                )
            })?;

        println!(
            "✓ Registered '{}' under project '{}'",
            codebase.name, project
        );
        println!(
            "  Saved to: ~/.orchestra/projects/{}/{}.yaml",
            project, codebase.name
        );
        Ok(())
    }
}
