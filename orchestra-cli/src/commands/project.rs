//! `orchestra project list` and `orchestra project add <name>`

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use orchestra_core::{
    registry,
    types::{CodebaseName, ProjectName},
};

use super::super::ProjectTypeArg;

/// Manage codebases within the active registry.
#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// List all registered codebases grouped by project.
    List,

    /// Add a new codebase to a project directory.
    Add(AddArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Codebase name (e.g. "payments", "dashboard").
    pub name: String,

    /// Project group to add the codebase under.
    /// If omitted and only one project exists, that project is used automatically.
    #[arg(long = "project", short = 'p')]
    pub project: Option<String>,

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
    let codebases = registry::list_codebases()
        .context("failed to load registry — run `orchestra init` first")?;

    if codebases.is_empty() {
        println!("No codebases registered.");
        println!("Run: orchestra init <path> --project <name>");
        return Ok(());
    }

    // Group by project (already sorted deterministically by list_codebases_at)
    let mut current_project = String::new();
    for (project, codebase) in &codebases {
        if project.0 != current_project {
            println!("\nProject: {}", project);
            current_project = project.0.clone();
        }
        println!("  {} ({})", codebase.name, codebase.path.display());
        for p in &codebase.projects {
            println!("    - {} [{}]", p.name, p.project_type);
        }
    }

    Ok(())
}

fn add(args: AddArgs) -> Result<()> {
    // Resolve which project to add to
    let project = match args.project {
        Some(p) => ProjectName::from(p),
        None => {
            let projects = registry::list_project_names().context("failed to read project list")?;
            match projects.len() {
                0 => {
                    return Err(anyhow::anyhow!(
                        "No projects found. Run `orchestra init <path> --project <name>` first."
                    ))
                }
                1 => projects.into_iter().next().expect("len == 1"),
                _ => {
                    let names: Vec<&str> = projects.iter().map(|p| p.0.as_str()).collect();
                    return Err(anyhow::anyhow!(
                        "Multiple projects found ({}). Specify --project <name>.",
                        names.join(", ")
                    ));
                }
            }
        }
    };

    let project_type = args.project_type.unwrap_or_default().into();
    let codebase = registry::add_codebase(
        &project,
        CodebaseName::from(args.name.clone()),
        project_type,
    )
    .with_context(|| {
        format!(
            "failed to add '{}' to project '{}' — run `orchestra init` first",
            args.name, project
        )
    })?;

    println!("✓ Added '{}' to project '{}'", codebase.name, project);
    Ok(())
}
