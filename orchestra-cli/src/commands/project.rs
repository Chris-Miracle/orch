//! `orchestra project list` and `orchestra project add <name>`

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use orchestra_core::{registry, types::ProjectName};

use super::super::ProjectTypeArg;

/// Manage projects within the active registry.
#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// List all projects across all tracked codebases.
    List,

    /// Add a new project to the first registered codebase.
    Add(AddArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Project name (e.g. "api", "dashboard").
    pub name: String,

    /// Project category: backend | frontend | mobile | ml. Defaults to backend.
    #[arg(long = "type", short = 't', value_name = "TYPE")]
    pub project_type: Option<ProjectTypeArg>,
}

pub fn run(cmd: ProjectCommand) -> Result<()> {
    match cmd {
        ProjectCommand::List => list(),
        ProjectCommand::Add(args) => add(args),
    }
}

fn list() -> Result<()> {
    let registry = registry::load()
        .context("failed to load registry — run `orchestra init` first")?;

    if registry.codebases.is_empty() {
        println!("No codebases registered.");
        return Ok(());
    }

    for codebase in &registry.codebases {
        println!("Codebase: {} ({})", codebase.name, codebase.path.display());
        if codebase.projects.is_empty() {
            println!("  (no projects)");
        } else {
            for project in &codebase.projects {
                println!("  - {} [{}]", project.name, project.project_type);
            }
        }
    }

    Ok(())
}

fn add(args: AddArgs) -> Result<()> {
    let project_type = args.project_type.unwrap_or_default().into();
    let name = ProjectName::from(args.name.clone());

    registry::add_project(name, project_type)
        .with_context(|| format!("failed to add project '{}' — run `orchestra init` first", args.name))?;

    println!("✓ Added project '{}'", args.name);
    Ok(())
}
