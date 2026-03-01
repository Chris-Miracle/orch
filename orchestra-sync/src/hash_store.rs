//! Hash store — SHA-256-based idempotency tracking for synced files.
//!
//! Persists a `HashStoreFile` JSON document at
//! `<home>/.orchestra/hashes/<codebase_name>.json`.
//! Writes use the same atomic `.tmp` + rename pattern as the registry.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
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
    match serde_json::from_str::<HashStoreCompat>(&contents)? {
        HashStoreCompat::Structured(store) => Ok(HashStoreFile {
            synced_at: store.synced_at.unwrap_or_else(Utc::now),
            files: store.files,
        }),
        HashStoreCompat::Legacy(files) => Ok(HashStoreFile {
            synced_at: Utc::now(),
            files,
        }),
    }
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
}
