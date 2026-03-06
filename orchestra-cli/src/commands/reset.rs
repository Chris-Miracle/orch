//! `orchestra reset` — wipe Orchestra and all its managed files, then reinstall clean.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use orchestra_core::registry;
use orchestra_renderer::engine::{backup_dir, legacy_orchestra_dirs, orchestra_dir, AgentKind};
use orchestra_sync::{load_backup_manifest, restore_from_backup};

/// Arguments for `orchestra reset`.
#[derive(Args, Debug)]
pub struct ResetArgs {
    /// Required safety flag — you must explicitly confirm this destructive operation.
    #[arg(long)]
    pub confirm: bool,

    /// Also restore all pre-onboard agent files from backups before wiping.
    #[arg(long)]
    pub restore_backups: bool,
}

impl ResetArgs {
    pub fn run(self) -> Result<()> {
        if !self.confirm {
            eprintln!("orchestra reset requires --confirm to prevent accidental data loss.");
            eprintln!();
            eprintln!("  orchestra reset --confirm");
            eprintln!();
            eprintln!("This will:");
            eprintln!("  • Remove all Orchestra-managed agent files from every registered codebase");
            eprintln!("  • Remove all project-local orchestra/ directories from every registered codebase");
            eprintln!("  • Wipe ~/.orchestra/ (registry, hashes, channel, daemon socket, etc.)");
            eprintln!();
            eprintln!("Optionally, pass --restore-backups to restore pre-onboard agent files first.");
            std::process::exit(1);
        }

        let home = dirs::home_dir().context("could not determine home directory")?;

        println!("🎼 Orchestra Reset");
        println!();

        // Collect all codebases before we destroy the registry
        let codebases = registry::list_codebases_at(&home)
            .unwrap_or_default();

        if codebases.is_empty() {
            println!("  No registered codebases found.");
        } else {
            println!("  Found {} registered codebase(s):", codebases.len());
            for (project, cb) in &codebases {
                println!("    • {} / {} ({})", project, cb.name, cb.path.display());
            }
            println!();

            for (project, codebase) in &codebases {
                let codebase_path = &codebase.path;

                if !codebase_path.exists() {
                    println!("  Skipping '{}' — path no longer exists.", codebase.name);
                    continue;
                }

                println!("  Processing '{}'...", codebase.name);

                // Optionally restore backups
                if self.restore_backups {
                    let backup_manifest = backup_dir(codebase_path).join("manifest.json");
                    if backup_manifest.exists() {
                        match restore_from_backup(codebase_path) {
                            Ok(n) => println!("    ✓ Restored {} files from backup.", n),
                            Err(e) => eprintln!("    ⚠ Could not restore backups: {}", e),
                        }
                    }
                }

                // Remove Orchestra-managed agent files
                let protected = protected_restore_paths(codebase_path);
                let mut removed = 0usize;
                for path in managed_cleanup_paths(codebase_path)
                    .into_iter()
                    .filter(|p| !protected.iter().any(|protected| protected == p))
                    .filter(|p: &PathBuf| p.exists())
                {
                    if std::fs::remove_file(&path).is_ok() {
                        removed += 1;
                    }
                }
                println!("    ✓ Removed {} managed agent files.", removed);

                // Remove current and legacy Orchestra directories inside codebase
                let orch_dir = orchestra_dir(codebase_path);
                if orch_dir.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&orch_dir) {
                        eprintln!("    ⚠ Could not remove orchestra/: {}", e);
                    } else {
                        println!("    ✓ Removed orchestra/.");
                    }
                }
                for legacy_dir in legacy_orchestra_dirs(codebase_path) {
                    if legacy_dir.exists() {
                        if let Err(e) = std::fs::remove_dir_all(&legacy_dir) {
                            eprintln!("    ⚠ Could not remove legacy orchestra/: {}", e);
                        } else {
                            println!("    ✓ Removed legacy orchestra/.");
                        }
                    }
                }

                let _ = project; // suppress unused variable warning
            }
        }

        println!();

        // Wipe ~/.orchestra/ — the global registry  
        let orchestra_home = home.join(".orchestra");
        if orchestra_home.exists() {
            std::fs::remove_dir_all(&orchestra_home)
                .with_context(|| format!("failed to remove {}", orchestra_home.display()))?;
            println!("  ✓ Removed ~/.orchestra/ (registry, hashes, channel).");
        } else {
            println!("  ~/.orchestra/ not found — nothing to wipe.");
        }

        println!();
        println!("✓ Orchestra has been fully reset.");
        println!();
        println!("  To set up a fresh installation:");
        println!("    orchestra onboard");
        println!(
            "  To reinstall the latest version:\n    curl -fsSL https://raw.githubusercontent.com/Chris-Miracle/orch/main/install.sh | sh"
        );

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
