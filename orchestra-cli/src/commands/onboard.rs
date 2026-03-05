//! `orchestra onboard` — interactive onboarding and bootstrap.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use orchestra_core::{
    registry,
    types::{ProjectName, ProjectType},
};
use orchestra_detector::{detect_stack, scan_agent_files};
use orchestra_sync::{
    backup_agent_files, managed_agent_paths, pipeline, remove_agent_files_protected, BackupItem,
    SyncScope,
};

/// Arguments for `orchestra onboard`.
#[derive(Args, Debug)]
pub struct OnboardArgs {
    /// Optional path to codebase root (defaults to current directory).
    pub path: Option<PathBuf>,

    /// Project group name. If omitted, interactive prompt is used.
    #[arg(long, short = 'p')]
    pub project: Option<String>,

    /// Accept detected/default project type without prompt.
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Re-run onboarding even if this path is already registered.
    #[arg(long)]
    pub force: bool,
}

impl OnboardArgs {
    pub fn run(self) -> Result<()> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let raw_path = self.path.unwrap_or_else(|| PathBuf::from("."));
        let codebase_path = raw_path
            .canonicalize()
            .with_context(|| format!("cannot resolve path '{}'", raw_path.display()))?;

        let codebases = registry::list_codebases_at(&home).context("failed to read registry")?;
        if let Some((project, cb)) = codebases.iter().find(|(_, cb)| cb.path == codebase_path) {
            if !self.force {
                println!(
                    "Already onboarded as '{}' under project '{}'.",
                    cb.name, project
                );
                println!("Use --force to re-run onboarding workflow.");
                return Ok(());
            }
        }

        let project_type = resolve_project_type(&codebase_path, self.yes)?;
        let project_name = resolve_project_name(self.project, &home, &codebase_path)?;
        let project_key = ProjectName::from(project_name.clone());

        let mut codebase = registry::init_at(
            codebase_path.clone(),
            project_key.clone(),
            Some(project_type),
            &home,
        )
        .with_context(|| {
            format!(
                "failed to init '{}' under project '{}'",
                codebase_path.display(),
                project_name
            )
        })?;

        let hits = scan_agent_files(&codebase_path).context("failed to scan existing agent files")?;
        if !hits.is_empty() {
            println!("Found {} existing agent file/folder entries.", hits.len());

            let (legacy_conventions, legacy_notes) = extract_legacy_hints(&hits);
            for convention in legacy_conventions {
                if !codebase.conventions.iter().any(|c| c == &convention) {
                    codebase.conventions.push(convention);
                }
            }
            for note in legacy_notes {
                if !codebase.notes.iter().any(|n| n == &note) {
                    codebase.notes.push(note);
                }
            }
            registry::save_codebase_at(&home, &project_key, &codebase)
                .context("failed saving merged legacy hints to registry")?;

            let backup_items: Vec<BackupItem> = hits
                .iter()
                .map(|hit| BackupItem {
                    provider: hit.provider.clone(),
                    path: hit.path.clone(),
                    is_subagent: hit.is_subagent,
                })
                .collect();

            let manifest = backup_agent_files(&codebase_path, &backup_items)
                .context("failed while backing up existing agent files")?;

            let protected = managed_agent_paths(&[(project_key.clone(), codebase.clone())]);
            remove_agent_files_protected(&backup_items, &protected)
                .context("failed while cleaning old agent files after backup")?;

            println!(
                "Backed up {} entries to {}",
                manifest.files.len(),
                codebase_path.join(".orchestra/backup").display()
            );
        }

        let gitignore_path = codebase_path.join(".orchestra").join(".gitignore");
        if !gitignore_path.exists() {
            let _ = std::fs::create_dir_all(codebase_path.join(".orchestra"));
            let _ = std::fs::write(&gitignore_path, "backup/\n");
        }

        let mut results = pipeline::run(&home, SyncScope::Codebase(codebase.name.0.clone()), false)
            .with_context(|| format!("sync failed for '{}'", codebase.name))?;

        if let Some(result) = results.pop() {
            let changed = result
                .writes
                .iter()
                .filter(|w| !matches!(w, orchestra_sync::WriteResult::Unchanged { .. }))
                .count();
            println!("Synced '{}' ({} file updates).", result.codebase_name, changed);
        }

        println!(
            "✓ Onboarded '{}' under project '{}'.",
            codebase.name, project_name
        );
        println!(
            "  Pilot entrypoint: {}",
            codebase.path.join(".orchestra/pilot.md").display()
        );
        Ok(())
    }
}

fn resolve_project_type(codebase_path: &std::path::Path, auto_yes: bool) -> Result<ProjectType> {
    let detected = detect_stack(codebase_path).ok();
    let mut selected = detected
        .as_ref()
        .map(|d| d.project_type.clone())
        .unwrap_or(ProjectType::Backend);

    if let Some(stack) = detected {
        println!(
            "Detected stack: {}{} -> {}",
            stack.primary_language,
            stack
                .framework
                .as_ref()
                .map(|f| format!(" / {f}"))
                .unwrap_or_default(),
            stack.project_type
        );
    } else {
        println!("Could not detect stack confidently; defaulting to backend.");
    }

    if auto_yes {
        return Ok(selected);
    }

    let confirm = prompt("Use this project type? [Y/n/change]: ")?;
    let confirm = confirm.trim().to_ascii_lowercase();
    if confirm == "n" || confirm == "no" || confirm == "change" || confirm == "c" {
        selected = prompt_project_type()?;
    }

    Ok(selected)
}

fn prompt_project_type() -> Result<ProjectType> {
    loop {
        let input = prompt("Project type (backend|frontend|mobile|ml): ")?;
        match input.trim().to_ascii_lowercase().as_str() {
            "backend" => return Ok(ProjectType::Backend),
            "frontend" => return Ok(ProjectType::Frontend),
            "mobile" => return Ok(ProjectType::Mobile),
            "ml" => return Ok(ProjectType::Ml),
            _ => println!("Please enter one of: backend, frontend, mobile, ml"),
        }
    }
}

fn resolve_project_name(
    provided: Option<String>,
    home: &std::path::Path,
    codebase_path: &std::path::Path,
) -> Result<String> {
    if let Some(name) = provided {
        return Ok(name);
    }

    let existing = registry::list_project_names_at(home).context("failed to list project names")?;
    let default_name = codebase_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    if existing.is_empty() {
        let input = prompt(&format!("Project name [{}]: ", default_name))?;
        let trimmed = input.trim();
        return Ok(if trimmed.is_empty() {
            default_name
        } else {
            trimmed.to_string()
        });
    }

    println!(
        "Existing projects: {}",
        existing
            .iter()
            .map(|p| p.0.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Enter an existing project name to attach, or a new name to create one.");

    loop {
        let input = prompt(&format!("Project name [{}]: ", default_name))?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default_name.clone());
        }
        return Ok(trimmed.to_string());
    }
}

fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush().context("failed to flush stdout")?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read user input")?;
    Ok(input)
}

fn extract_legacy_hints(hits: &[orchestra_detector::AgentFileHit]) -> (Vec<String>, Vec<String>) {
    let mut conventions = Vec::new();
    let mut notes = Vec::new();

    for hit in hits {
        if hit.path.is_file() {
            collect_hints_from_file(&hit.path, &mut conventions, &mut notes);
            continue;
        }
        if hit.path.is_dir() {
            collect_hints_from_dir(&hit.path, &mut conventions, &mut notes);
        }
    }

    conventions.sort();
    conventions.dedup();
    notes.sort();
    notes.dedup();

    conventions.truncate(20);
    notes.truncate(20);

    (conventions, notes)
}

fn collect_hints_from_dir(path: &std::path::Path, conventions: &mut Vec<String>, notes: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_hints_from_dir(&p, conventions, notes);
        } else {
            collect_hints_from_file(&p, conventions, notes);
        }
    }
}

fn collect_hints_from_file(path: &std::path::Path, conventions: &mut Vec<String>, notes: &mut Vec<String>) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !matches!(ext, "md" | "mdc" | "txt" | "json" | "yaml" | "yml") {
        return;
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    for line in content.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let lowered = line.to_ascii_lowercase();
        if line.starts_with("-")
            && (lowered.contains("always")
                || lowered.contains("never")
                || lowered.contains("must")
                || lowered.contains("do not")
                || lowered.contains("don't"))
        {
            conventions.push(line.trim_start_matches('-').trim().to_string());
            continue;
        }

        if line.starts_with('#') || line.starts_with("note") || line.starts_with("tip") {
            notes.push(format!("[legacy:{}] {}", path.display(), line));
        }
    }
}
