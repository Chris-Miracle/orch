//! Phase 03 staleness signal detection.
//!
//! Signal precedence:
//! 1. `NeverSynced` (hash store missing or empty)
//! 2. `Stale` (registry changed after `synced_at`, or managed files missing)
//! 3. `Modified` (rendered files changed since last sync hash)
//! 4. `Orphan` (managed files present but not tracked in hash store)
//! 5. `Current`

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use orchestra_core::{
    registry,
    types::{Codebase, ProjectName},
};
use orchestra_renderer::AgentKind;

use crate::{error::io_err, hash_store, SyncError};

/// Phase 03 staleness classification for a codebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StalenessSignal {
    NeverSynced,
    Current,
    Stale { reason: String },
    Modified { files: Vec<PathBuf> },
    Orphan { files: Vec<PathBuf> },
}

/// Check a codebase for staleness against registry metadata, hash store, and
/// managed file presence.
pub fn check(
    home: &Path,
    project: &ProjectName,
    codebase: &Codebase,
) -> Result<StalenessSignal, SyncError> {
    let managed = managed_paths(codebase);
    let mut managed_keys = BTreeSet::new();
    for path in &managed {
        managed_keys.insert(path.to_string_lossy().to_string());
    }

    // First-run handling: no hash file or no tracked hashes is "never synced",
    // not "stale".
    let store_path = hash_store::store_path_at(home, &codebase.name.0);
    let store_exists = store_path.exists();
    let store = hash_store::load_at(home, &codebase.name.0)?;
    if !store_exists || store.files.is_empty() {
        return Ok(StalenessSignal::NeverSynced);
    }

    let mut missing = Vec::new();
    for path in &managed {
        match std::fs::metadata(path) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {
                missing.push(relative_to_codebase(path, codebase));
            }
            Err(err) => return Err(io_err(path, err)),
        }
    }

    if !missing.is_empty() {
        sort_and_dedup_paths(&mut missing);
        return Ok(StalenessSignal::Stale {
            reason: format!(
                "missing {} managed file(s): {}",
                missing.len(),
                preview_files(&missing),
            ),
        });
    }

    // Freshness is based on hash-store sync time, not rendered file mtimes.
    let registry_path = registry::codebase_path_at(home, project, &codebase.name);
    let registry_meta = std::fs::metadata(&registry_path).map_err(|e| io_err(&registry_path, e))?;
    let registry_mtime = registry_meta
        .modified()
        .map_err(|e| io_err(&registry_path, e))?;
    let registry_ts = unix_duration(registry_mtime);
    let synced_ts = datetime_to_unix_duration(store.synced_at);
    if registry_ts > synced_ts {
        return Ok(StalenessSignal::Stale {
            reason: format!(
                "registry changed {} ago",
                format_system_time_age(registry_mtime)
            ),
        });
    }

    let mut modified = Vec::new();
    for path in &managed {
        let key = path.to_string_lossy().to_string();
        let Some(expected_hash) = store.files.get(&key) else {
            continue;
        };
        let current_hash = hash_file(path)?;
        if &current_hash != expected_hash {
            modified.push(relative_to_codebase(path, codebase));
        }
    }
    if !modified.is_empty() {
        sort_and_dedup_paths(&mut modified);
        return Ok(StalenessSignal::Modified { files: modified });
    }

    let mut orphan = Vec::new();
    for path in &managed {
        let key = path.to_string_lossy().to_string();
        if path.exists() && !store.files.contains_key(&key) {
            orphan.push(relative_to_codebase(path, codebase));
        }
    }
    for key in store.files.keys() {
        if managed_keys.contains(key) {
            continue;
        }
        let path = PathBuf::from(key);
        if path.exists() {
            orphan.push(relative_to_codebase(&path, codebase));
        }
    }
    if !orphan.is_empty() {
        sort_and_dedup_paths(&mut orphan);
        return Ok(StalenessSignal::Orphan { files: orphan });
    }

    Ok(StalenessSignal::Current)
}

/// Format age from a filesystem timestamp.
pub fn format_system_time_age(timestamp: SystemTime) -> String {
    let age = SystemTime::now()
        .duration_since(timestamp)
        .unwrap_or_default();
    format_duration(age)
}

/// Format age from a chrono timestamp (hash store `synced_at`).
pub fn format_datetime_age(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let age = now.signed_duration_since(timestamp).num_seconds().max(0) as u64;
    format_seconds(age)
}

fn managed_paths(codebase: &Codebase) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for agent in AgentKind::all() {
        paths.extend(agent.output_paths(&codebase.path));
    }
    paths
}

fn hash_file(path: &Path) -> Result<String, SyncError> {
    let content = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let normalized = content.replace("\r\n", "\n");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

fn unix_duration(timestamp: SystemTime) -> Duration {
    timestamp.duration_since(UNIX_EPOCH).unwrap_or_default()
}

fn datetime_to_unix_duration(timestamp: DateTime<Utc>) -> Duration {
    let secs = timestamp.timestamp().max(0) as u64;
    Duration::new(secs, timestamp.timestamp_subsec_nanos())
}

fn relative_to_codebase(path: &Path, codebase: &Codebase) -> PathBuf {
    path.strip_prefix(&codebase.path)
        .unwrap_or(path)
        .to_path_buf()
}

fn format_duration(duration: Duration) -> String {
    format_seconds(duration.as_secs())
}

fn format_seconds(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    if seconds < 60 * 60 {
        return format!("{}m", seconds / 60);
    }
    if seconds < 60 * 60 * 24 {
        return format!("{}h", seconds / (60 * 60));
    }
    format!("{}d", seconds / (60 * 60 * 24))
}

fn sort_and_dedup_paths(paths: &mut Vec<PathBuf>) {
    paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    paths.dedup();
}

fn preview_files(paths: &[PathBuf]) -> String {
    let mut shown: Vec<String> = paths
        .iter()
        .take(3)
        .map(|p| p.display().to_string())
        .collect();
    if paths.len() > shown.len() {
        shown.push(format!("+{} more", paths.len() - shown.len()));
    }
    shown.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, thread::sleep};

    use orchestra_core::{
        registry,
        types::{ProjectName, ProjectType},
    };
    use tempfile::TempDir;

    use crate::{hash_store::HashStoreFile, sync_codebase};

    fn setup_codebase() -> (
        TempDir,
        TempDir,
        String,
        ProjectName,
        orchestra_core::types::Codebase,
    ) {
        let home = TempDir::new().expect("home");
        let workspace = TempDir::new().expect("workspace");
        let codebase_dir = workspace.path().join("copnow_api");
        fs::create_dir_all(&codebase_dir).expect("mkdir");

        let project = ProjectName::from("copnow");
        registry::init_at(
            codebase_dir,
            project.clone(),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");

        sync_codebase("copnow_api", home.path(), false).expect("sync");
        let (actual_project, codebase) = registry::list_codebases_at(home.path())
            .expect("list")
            .into_iter()
            .find(|(_, cb)| cb.name.0 == "copnow_api")
            .expect("codebase");
        (
            home,
            workspace,
            "copnow_api".to_string(),
            actual_project,
            codebase,
        )
    }

    #[test]
    fn returns_current_after_sync() {
        let (home, _workspace, _name, project, codebase) = setup_codebase();
        let signal = check(home.path(), &project, &codebase).expect("check");
        assert_eq!(signal, StalenessSignal::Current);
    }

    #[test]
    fn returns_never_synced_when_hash_store_missing() {
        let home = TempDir::new().expect("home");
        let workspace = TempDir::new().expect("workspace");
        let codebase_dir = workspace.path().join("copnow_api");
        fs::create_dir_all(&codebase_dir).expect("mkdir");
        let project = ProjectName::from("copnow");
        registry::init_at(
            codebase_dir,
            project.clone(),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");

        let (_, codebase) = registry::list_codebases_at(home.path())
            .expect("list")
            .into_iter()
            .next()
            .expect("codebase");
        let signal = check(home.path(), &project, &codebase).expect("check");
        assert_eq!(signal, StalenessSignal::NeverSynced);
    }

    #[test]
    fn returns_stale_when_registry_newer_than_managed_files() {
        let (home, _workspace, _name, project, codebase) = setup_codebase();

        sleep(std::time::Duration::from_millis(1100));
        let registry_path = registry::codebase_path_at(home.path(), &project, &codebase.name);
        let yaml = fs::read_to_string(&registry_path).expect("read");
        fs::write(&registry_path, yaml).expect("touch registry");

        let signal = check(home.path(), &project, &codebase).expect("check");
        match signal {
            StalenessSignal::Stale { reason } => assert!(reason.contains("registry")),
            other => panic!("expected stale, got {other:?}"),
        }
    }

    #[test]
    fn returns_modified_when_managed_file_is_edited() {
        let (home, _workspace, _name, project, codebase) = setup_codebase();
        let target = codebase.path.join("CLAUDE.md");
        fs::write(&target, "manually edited\n").expect("edit");

        let signal = check(home.path(), &project, &codebase).expect("check");
        match signal {
            StalenessSignal::Modified { files } => {
                assert!(files.iter().any(|p| p == &PathBuf::from("CLAUDE.md")));
            }
            other => panic!("expected modified, got {other:?}"),
        }
    }

    #[test]
    fn returns_orphan_when_hash_entry_missing_for_existing_managed_file() {
        let (home, _workspace, name, project, codebase) = setup_codebase();
        let mut store = hash_store::load_at(home.path(), &name).expect("load store");
        store.files.remove(
            &codebase
                .path
                .join("CLAUDE.md")
                .to_string_lossy()
                .to_string(),
        );
        hash_store::save_at(home.path(), &name, &store).expect("save store");

        let signal = check(home.path(), &project, &codebase).expect("check");
        match signal {
            StalenessSignal::Orphan { files } => {
                assert!(files.iter().any(|p| p == &PathBuf::from("CLAUDE.md")));
            }
            other => panic!("expected orphan, got {other:?}"),
        }
    }

    #[test]
    fn datetime_age_and_system_age_are_compact() {
        let now = Utc::now();
        assert_eq!(format_datetime_age(now), "0s");

        let time = SystemTime::now() - Duration::from_secs(65);
        assert_eq!(format_system_time_age(time), "1m");
    }

    #[test]
    fn stale_on_missing_managed_file() {
        let (home, _workspace, _name, project, codebase) = setup_codebase();
        let target = codebase.path.join("CLAUDE.md");
        fs::remove_file(&target).expect("remove");

        let signal = check(home.path(), &project, &codebase).expect("check");
        match signal {
            StalenessSignal::Stale { reason } => {
                assert!(reason.contains("missing"));
                assert!(reason.contains("CLAUDE.md"));
            }
            other => panic!("expected stale, got {other:?}"),
        }
    }

    #[test]
    fn orphan_detects_store_paths_outside_managed_set() {
        let (home, _workspace, name, project, codebase) = setup_codebase();

        let extra = codebase.path.join("legacy_agent.md");
        fs::write(&extra, "legacy").expect("write extra");
        let mut store = hash_store::load_at(home.path(), &name).expect("load");
        store
            .files
            .insert(extra.to_string_lossy().to_string(), "deadbeef".to_string());
        hash_store::save_at(home.path(), &name, &store).expect("save");

        let signal = check(home.path(), &project, &codebase).expect("check");
        match signal {
            StalenessSignal::Orphan { files } => {
                assert!(files.iter().any(|p| p == &PathBuf::from("legacy_agent.md")));
            }
            other => panic!("expected orphan, got {other:?}"),
        }
    }

    #[test]
    fn orphan_after_hash_store_file_deleted_and_files_exist() {
        let (home, _workspace, name, project, codebase) = setup_codebase();
        let path = hash_store::store_path_at(home.path(), &name);
        fs::remove_file(&path).expect("delete store");

        let signal = check(home.path(), &project, &codebase).expect("check");
        assert_eq!(signal, StalenessSignal::NeverSynced);
    }

    #[test]
    fn orphan_detects_untracked_existing_file_even_with_legacy_store_shape() {
        let (home, _workspace, _name, project, codebase) = setup_codebase();
        let store_path = hash_store::store_path_at(home.path(), "copnow_api");
        let legacy = HashStoreFile {
            synced_at: Utc::now(),
            files: std::collections::HashMap::new(),
        };
        fs::write(
            store_path,
            serde_json::to_string(&legacy.files).expect("serialize"),
        )
        .expect("write");

        let signal = check(home.path(), &project, &codebase).expect("check");
        assert_eq!(signal, StalenessSignal::NeverSynced);
    }
}
