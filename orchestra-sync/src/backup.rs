use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    let backup_root = codebase_root.join(".orchestra").join("backup");
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
        files,
    };

    let manifest_path = backup_root.join("manifest.json");
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, json)
        .map_err(|e| SyncError::Io { path: manifest_path, source: e })?;

    Ok(manifest)
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
