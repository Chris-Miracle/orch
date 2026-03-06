//! `orchestra offboard` — restore pre-onboard state and deregister a codebase.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use orchestra_core::{registry, types::ProjectName};
use orchestra_renderer::engine::{backup_dir, legacy_orchestra_dirs, orchestra_dir, AgentKind};
use orchestra_sync::{load_backup_manifest, restore_from_backup};

/// Arguments for `orchestra offboard`.
#[derive(Args, Debug)]
pub struct OffboardArgs {
    /// Optional path to codebase root (defaults to current directory).
    pub path: Option<PathBuf>,

    /// Project group name containing this codebase.
    #[arg(long, short = 'p')]
    pub project: Option<String>,

    /// Skip confirmation prompt.
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Treat this as a recent-onboarding revert.
    #[arg(long)]
    pub recent: bool,
}

impl OffboardArgs {
    pub fn run(self) -> Result<()> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let raw_path = self.path.unwrap_or_else(|| PathBuf::from("."));
        let codebase_path = raw_path
            .canonicalize()
            .with_context(|| format!("cannot resolve path '{}'", raw_path.display()))?;

        // Find the registered codebase
        let codebases = registry::list_codebases_at(&home).context("failed to read registry")?;
        let (project_key, codebase) = codebases
            .iter()
            .find(|(_, cb)| cb.path == codebase_path)
            .map(|(p, cb)| (p.clone(), cb.clone()))
            .or_else(|| {
                // Try matching by project flag if provided
                if let Some(ref proj) = self.project {
                    let key = ProjectName::from(proj.clone());
                    codebases
                        .iter()
                        .find(|(p, _)| p == &key)
                        .map(|(p, cb)| (p.clone(), cb.clone()))
                } else {
                    None
                }
            })
            .with_context(|| {
                format!(
                    "no registered codebase found at '{}'. Use `orchestra project list` to see registered codebases.",
                    codebase_path.display()
                )
            })?;

        if self.recent {
            println!(
                "Preparing to revert recent onboarding for '{}' under project '{}'.",
                codebase.name, project_key
            );
        } else {
            println!("Preparing to offboard '{}' under project '{}'.", codebase.name, project_key);
        }
        println!();

        // Show what will happen
        let backup_manifest = backup_dir(&codebase_path).join("manifest.json");
        if backup_manifest.exists() {
            println!("  ✓ Backup found — pre-onboard files will be restored.");
        } else {
            println!("  ⚠ No backup found at orchestra/backup/ — original files cannot be restored.");
        }

        // Count managed files that will be removed
        let protected = protected_restore_paths(&codebase_path);
        let managed: Vec<PathBuf> = managed_cleanup_paths(&codebase_path)
            .into_iter()
            .filter(|p| !protected.iter().any(|protected| protected == p))
            .filter(|p: &PathBuf| p.exists())
            .collect();
        println!("  ✓ {} Orchestra-managed agent files will be removed.", managed.len());
        println!("  ✓ orchestra/ controls and backup directories will be removed.");
        println!("  ✓ Codebase will be deregistered from the registry.");
        println!();

        if !self.yes {
            let confirm = prompt("Proceed with offboard? This cannot be undone. [y/N]: ")?;
            if !matches!(confirm.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                println!("Offboard cancelled.");
                return Ok(());
            }
        }

        // Step 1: Restore from backup
        if backup_manifest.exists() {
            let num_restored = restore_from_backup(&codebase_path)
                .context("failed to restore files from backup")?;
            println!("Restored {} files from backup.", num_restored);
        }

        // Step 2: Remove Orchestra-managed agent files
        let mut removed_count = 0usize;
        for path in managed {
            if path.exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    eprintln!("  Warning: could not remove {}: {}", path.display(), e);
                } else {
                    removed_count += 1;
                }
            }
        }
        // Remove any now-empty agent directories that Orchestra created
        prune_empty_agent_dirs(&codebase_path);
        println!("Removed {} Orchestra-managed files.", removed_count);

        // Step 3: Remove project-local Orchestra directories
        let project_orchestra_dir = orchestra_dir(&codebase_path);
        if project_orchestra_dir.exists() {
            std::fs::remove_dir_all(&project_orchestra_dir)
                .with_context(|| format!("failed to remove {}", project_orchestra_dir.display()))?;
            println!("Removed orchestra/ directory.");
        }
        for legacy_dir in legacy_orchestra_dirs(&codebase_path) {
            if legacy_dir.exists() {
                std::fs::remove_dir_all(&legacy_dir)
                    .with_context(|| format!("failed to remove {}", legacy_dir.display()))?;
                println!("Removed legacy orchestra/ directory.");
            }
        }

        // Step 4: Deregister from global registry
        registry::remove_codebase_at(&home, &project_key, &codebase.name)
            .context("failed to deregister codebase from registry")?;
        println!("Deregistered '{}' from registry.", codebase.name);

        println!();
        println!("✓ Offboarded '{}' successfully.", codebase.name);
        println!("  Your codebase is back to its pre-Orchestra state.");

        Ok(())
    }
}

fn managed_cleanup_paths(codebase_root: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for agent in AgentKind::all() {
        paths.extend(agent.output_paths(codebase_root));
        paths.extend(agent.legacy_output_paths(codebase_root));
    }
    paths.sort();
    paths.dedup();
    paths
}

fn protected_restore_paths(codebase_root: &std::path::Path) -> Vec<PathBuf> {
    let Ok(Some(manifest)) = load_backup_manifest(codebase_root) else {
        return Vec::new();
    };

    let mut paths: Vec<PathBuf> = manifest
        .files
        .into_iter()
        .map(|entry| codebase_root.join(entry.original_path))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Remove empty subdirectories that legacy Orchestra runs may have created (best-effort).
fn prune_empty_agent_dirs(codebase_root: &std::path::Path) {
    // Top-level dirs Orchestra may have created or extended
    let agent_dirs = [
        ".claude/agents",
        ".claude/rules",
        ".claude",
        ".cursor/rules",
        ".cursor/skills/orchestra-sync",
        ".cursor/skills",
        ".cursor",
        ".windsurf/rules",
        ".windsurf/skills/orchestra-sync",
        ".windsurf/skills",
        ".windsurf",
        ".github/instructions",
        ".codex/skills/orchestra-sync",
        ".codex/skills",
        ".codex",
        ".gemini/skills/orchestra-sync",
        ".gemini/skills",
        ".gemini",
        ".clinerules",
        ".agents/skills/orchestra-sync",
        ".agents/skills",
        ".agents",
        ".agent/rules",
        ".agent/skills/orchestra-sync",
        ".agent/skills",
        ".agent",
    ];

    // Prune leaf-first so parent dirs can be caught in subsequent iterations
    for rel in agent_dirs.iter() {
        let dir = codebase_root.join(rel);
        if dir.is_dir() {
            if let Ok(mut rd) = std::fs::read_dir(&dir) {
                if rd.next().is_none() {
                    let _ = std::fs::remove_dir(&dir);
                }
            }
        }
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
