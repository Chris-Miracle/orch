use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use orchestra_renderer::engine::{backup_dir, PROJECT_ORCHESTRA_DIR};

use crate::error::SyncError;

#[derive(Debug, Clone)]
pub struct BackupItem {
    pub provider: String,
    pub path: PathBuf,
    pub is_subagent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFileEntry {
    pub provider: String,
    pub original_path: String,
    pub backup_path: String,
    pub is_subagent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub created_at: DateTime<Utc>,
    pub layout_version: String,
    pub files: Vec<BackupFileEntry>,
}

fn copy_dir_recursive(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

pub fn backup_agent_files(
    codebase_root: &Path,
    items: &[BackupItem],
) -> Result<BackupManifest, SyncError> {
    let backup_root = backup_dir(codebase_root);
    fs::create_dir_all(&backup_root)
        .map_err(|e| SyncError::Io { path: backup_root.clone(), source: e })?;

    let mut files = Vec::new();

    for item in items {
        if !item.path.exists() {
            continue;
        }

        let relative = item
            .path
            .strip_prefix(codebase_root)
            .unwrap_or(item.path.as_path());
        let destination = backup_root.join(relative);

        if item.path.is_dir() {
            copy_dir_recursive(&item.path, &destination)
                .map_err(|e| SyncError::Io { path: item.path.clone(), source: e })?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| SyncError::Io { path: parent.to_path_buf(), source: e })?;
            }
            fs::copy(&item.path, &destination)
                .map_err(|e| SyncError::Io { path: item.path.clone(), source: e })?;
        }

        files.push(BackupFileEntry {
            provider: item.provider.clone(),
            original_path: relative.display().to_string(),
            backup_path: destination
                .strip_prefix(codebase_root)
                .unwrap_or(destination.as_path())
                .display()
                .to_string(),
            is_subagent: item.is_subagent,
        });
    }

    let manifest = BackupManifest {
        created_at: Utc::now(),
        layout_version: format!("{PROJECT_ORCHESTRA_DIR}/controls@v2"),
        files,
    };

    let manifest_path = backup_root.join("manifest.json");
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, json)
        .map_err(|e| SyncError::Io { path: manifest_path, source: e })?;

    Ok(manifest)
}

pub fn load_backup_manifest(codebase_root: &Path) -> Result<Option<BackupManifest>, SyncError> {
    let manifest_path = backup_dir(codebase_root).join("manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }

    let json = fs::read_to_string(&manifest_path)
        .map_err(|e| SyncError::Io { path: manifest_path.clone(), source: e })?;
    let manifest = serde_json::from_str(&json)
        .map_err(SyncError::Json)?;
    Ok(Some(manifest))
}

/// Restore agent files from the `orchestra/backup/` directory using `manifest.json`.
///
/// Reads the backup manifest written by [`backup_agent_files`] and copies
/// each backed-up file back to its original relative location inside the
/// codebase. Overwrites existing files. Returns the number of files restored.
pub fn restore_from_backup(codebase_root: &Path) -> Result<usize, SyncError> {
    let Some(manifest) = load_backup_manifest(codebase_root)? else {
        return Ok(0);
    };

    let mut restored_count = 0;

    for entry in manifest.files {
        let backup_file = codebase_root.join(&entry.backup_path);
        let target_file = codebase_root.join(&entry.original_path);

        if backup_file.exists() && backup_file.is_file() {
            if let Some(parent) = target_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::copy(&backup_file, &target_file).is_ok() {
                restored_count += 1;
            }
        } else if backup_file.exists() && backup_file.is_dir() {
            if copy_dir_recursive(&backup_file, &target_file).is_ok() {
                restored_count += 1;
            }
        }
    }

    Ok(restored_count)
}

pub fn remove_agent_files(items: &[BackupItem]) -> Result<(), SyncError> {
    remove_agent_files_protected(items, &[])
}

pub fn remove_agent_files_protected(
    items: &[BackupItem],
    protected_paths: &[PathBuf],
) -> Result<(), SyncError> {
    let protected: Vec<PathBuf> = protected_paths
        .iter()
        .map(|p| fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
        .collect();

    for item in items {
        if !item.path.exists() {
            continue;
        }

        let canonical = fs::canonicalize(&item.path).unwrap_or_else(|_| item.path.clone());
        if protected.iter().any(|p| p == &canonical) {
            continue;
        }

        if item.path.is_dir() {
            prune_dir(&item.path, &protected)?;
        } else {
            fs::remove_file(&item.path)
                .map_err(|e| SyncError::Io { path: item.path.clone(), source: e })?;
        }
    }
    Ok(())
}

fn prune_dir(dir: &Path, protected: &[PathBuf]) -> Result<(), SyncError> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|e| SyncError::Io {
        path: dir.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| SyncError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

        if protected.iter().any(|p| p == &canonical) {
            continue;
        }

        if path.is_dir() {
            prune_dir(&path, protected)?;
            if fs::read_dir(&path)
                .map_err(|e| SyncError::Io {
                    path: path.clone(),
                    source: e,
                })?
                .next()
                .is_none()
            {
                fs::remove_dir(&path).map_err(|e| SyncError::Io {
                    path: path.clone(),
                    source: e,
                })?;
            }
        } else {
            fs::remove_file(&path).map_err(|e| SyncError::Io {
                path: path.clone(),
                source: e,
            })?;
        }
    }

    Ok(())
}
