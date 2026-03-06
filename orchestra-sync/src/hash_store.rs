//! Hash store — SHA-256-based idempotency tracking for synced files.
//!
//! Persists a `HashStoreFile` JSON document at
//! `<home>/.orchestra/hashes/<codebase_name>.json`.
//! Writes use the same atomic `.tmp` + rename pattern as the registry.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use orchestra_core::registry;
use orchestra_renderer::engine::{control_dir, guide_path, pilot_path, AgentKind};
use serde::{Deserialize, Serialize};

use crate::error::{io_err, SyncError};

/// In-memory hash store: maps relative file path strings to their last
/// synced SHA-256 hex digest.
pub type HashStore = HashMap<String, String>;

/// On-disk hash store payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HashStoreFile {
    pub synced_at: DateTime<Utc>,
    pub files: HashStore,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum HashStoreCompat {
    Structured(HashStoreStructuredCompat),
    Legacy(HashStore),
}

#[derive(Debug, Deserialize)]
struct HashStoreStructuredCompat {
    pub synced_at: Option<DateTime<Utc>>,
    pub files: HashStore,
}

/// Path to the hash store JSON for a given codebase, rooted at `home`.
///
/// `~/.orchestra/hashes/<codebase_name>.json`
pub fn store_path_at(home: &Path, codebase_name: &str) -> PathBuf {
    home.join(".orchestra")
        .join("hashes")
        .join(format!("{codebase_name}.json"))
}

/// Load the hash store for `codebase_name`.
///
/// Returns an empty store if the file does not yet exist.
pub fn load_at(home: &Path, codebase_name: &str) -> Result<HashStoreFile, SyncError> {
    let path = store_path_at(home, codebase_name);
    if !path.exists() {
        return Ok(HashStoreFile {
            synced_at: Utc::now(),
            files: HashMap::new(),
        });
    }
    let contents = std::fs::read_to_string(&path).map_err(|e| io_err(&path, e))?;
    let mut store = match serde_json::from_str::<HashStoreCompat>(&contents)? {
        HashStoreCompat::Structured(store) => HashStoreFile {
            synced_at: store.synced_at.unwrap_or_else(Utc::now),
            files: store.files,
        },
        HashStoreCompat::Legacy(files) => HashStoreFile {
            synced_at: Utc::now(),
            files,
        },
    };

    if let Some((_, codebase)) = registry::list_codebases_at(home)
        .ok()
        .and_then(|all| all.into_iter().find(|(_, cb)| cb.name.0 == codebase_name))
    {
        store.files = migrate_legacy_hash_keys(store.files, &codebase.path);
    }

    Ok(store)
}

fn migrate_legacy_hash_keys(files: HashStore, codebase_root: &Path) -> HashStore {
    let legacy_paths = legacy_managed_paths(codebase_root);
    let mut migrated = HashMap::new();

    for (key, digest) in files {
        let normalized = normalize_hash_key(&key, codebase_root, &legacy_paths)
            .unwrap_or_else(|| key.clone());

        match migrated.entry(normalized.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(digest);
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if normalized == key {
                    entry.insert(digest);
                }
            }
        }
    }

    migrated
}

fn normalize_hash_key(key: &str, codebase_root: &Path, legacy_paths: &[PathBuf]) -> Option<String> {
    let path = PathBuf::from(key);

    if path == pilot_path(codebase_root) || path == guide_path(codebase_root) {
        return Some(key.to_string());
    }

    let current_controls = control_dir(codebase_root);
    if path.starts_with(&current_controls) {
        return Some(key.to_string());
    }

    let relative = path.strip_prefix(codebase_root).ok()?;
    if relative == Path::new(".orchestra/pilot.md") {
        return Some(pilot_path(codebase_root).to_string_lossy().to_string());
    }
    if relative == Path::new(".orchestra/.guide.md") {
        return Some(guide_path(codebase_root).to_string_lossy().to_string());
    }
    if let Ok(stripped) = relative.strip_prefix(".orchestra/controls") {
        return Some(current_controls.join(stripped).to_string_lossy().to_string());
    }
    if let Ok(stripped) = relative.strip_prefix("orchestra/control") {
        return Some(current_controls.join(stripped).to_string_lossy().to_string());
    }

    if legacy_paths.iter().any(|candidate| candidate == &path) {
        return Some(current_controls.join(relative).to_string_lossy().to_string());
    }

    None
}

fn legacy_managed_paths(codebase_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for agent in AgentKind::all() {
        paths.extend(agent.legacy_output_paths(codebase_root));
    }
    paths.push(codebase_root.join(".orchestra").join("pilot.md"));
    paths.push(codebase_root.join(".orchestra").join(".guide.md"));
    paths.sort();
    paths.dedup();
    paths
}

/// Save the hash store for `codebase_name` atomically.
///
/// Writes to `<path>.tmp` then renames to `<path>`.
pub fn save_at(home: &Path, codebase_name: &str, store: &HashStoreFile) -> Result<(), SyncError> {
    let path = store_path_at(home, codebase_name);
    let Some(dir) = path.parent() else {
        return Err(io_err(
            path,
            std::io::Error::other("invalid hash store path"),
        ));
    };

    // Ensure the hashes directory exists.
    std::fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;

    let json = serde_json::to_string_pretty(store)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| io_err(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| io_err(&path, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestra_core::{
        registry,
        types::{ProjectName, ProjectType},
    };
    use tempfile::TempDir;

    #[test]
    fn empty_store_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let store = load_at(tmp.path(), "nonexistent").unwrap();
        assert!(store.files.is_empty());
    }

    #[test]
    fn roundtrip_save_load() {
        let tmp = TempDir::new().unwrap();
        let mut files = HashMap::new();
        files.insert("CLAUDE.md".to_string(), "deadbeef".to_string());
        files.insert(
            ".agent/rules/orchestra.md".to_string(),
            "cafebabe".to_string(),
        );
        let store = HashStoreFile {
            synced_at: Utc::now(),
            files,
        };

        save_at(tmp.path(), "myapp", &store).unwrap();
        let loaded = load_at(tmp.path(), "myapp").unwrap();
        assert_eq!(loaded.files, store.files);
    }

    #[test]
    fn tmp_file_cleaned_up_after_save() {
        let tmp = TempDir::new().unwrap();
        let store = HashStoreFile {
            synced_at: Utc::now(),
            files: HashMap::new(),
        };
        save_at(tmp.path(), "clean_test", &store).unwrap();
        let tmp_path = store_path_at(tmp.path(), "clean_test").with_extension("json.tmp");
        assert!(
            !tmp_path.exists(),
            "tmp file should be removed after atomic rename"
        );
    }

    #[test]
    fn load_legacy_flat_map_migrates_to_structured_store() {
        let tmp = TempDir::new().unwrap();
        let path = store_path_at(tmp.path(), "legacy");
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            &path,
            r#"{"CLAUDE.md":"deadbeef",".agent/rules/orchestra.md":"cafebabe"}"#,
        )
        .unwrap();

        let before = Utc::now();
        let loaded = load_at(tmp.path(), "legacy").unwrap();
        let after = Utc::now();

        assert_eq!(loaded.files.get("CLAUDE.md"), Some(&"deadbeef".to_string()));
        assert_eq!(
            loaded.files.get(".agent/rules/orchestra.md"),
            Some(&"cafebabe".to_string())
        );
        assert!(loaded.synced_at >= before && loaded.synced_at <= after);
    }

    #[test]
    fn load_structured_without_synced_at_sets_timestamp() {
        let tmp = TempDir::new().unwrap();
        let path = store_path_at(tmp.path(), "missing_synced_at");
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(&path, r#"{"files":{"CLAUDE.md":"deadbeef"}}"#).unwrap();

        let before = Utc::now();
        let loaded = load_at(tmp.path(), "missing_synced_at").unwrap();
        let after = Utc::now();

        assert_eq!(loaded.files.get("CLAUDE.md"), Some(&"deadbeef".to_string()));
        assert!(loaded.synced_at >= before && loaded.synced_at <= after);
    }

    #[test]
    fn load_migrates_legacy_visible_control_paths_to_controls_layout() {
        let home = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let codebase_dir = workspace.path().join("atlas_api");
        std::fs::create_dir_all(&codebase_dir).unwrap();
        registry::init_at(
            codebase_dir.clone(),
            ProjectName::from("atlas"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .unwrap();

        let path = store_path_at(home.path(), "atlas_api");
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        let legacy_key = codebase_dir
            .join("orchestra/control/CLAUDE.md")
            .to_string_lossy()
            .to_string();
        std::fs::write(
            &path,
            format!(
                r#"{{"synced_at":"2026-03-06T00:00:00Z","files":{{"{legacy_key}":"deadbeef"}}}}"#
            ),
        )
        .unwrap();

        let loaded = load_at(home.path(), "atlas_api").unwrap();
        let migrated_key = codebase_dir
            .join("orchestra/controls/CLAUDE.md")
            .to_string_lossy()
            .to_string();
        assert_eq!(loaded.files.get(&migrated_key), Some(&"deadbeef".to_string()));
        assert!(!loaded.files.contains_key(&legacy_key));
    }
}
