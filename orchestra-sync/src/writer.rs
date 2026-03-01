//! Atomic writer and sync orchestration.
//!
//! ## `atomic_write` — 7-step protocol
//!
//! 1. Render content (already done by caller).
//! 2. SHA-256 hash the rendered content.
//! 3. Load the hash store.
//! 4. Compare with stored hash → skip if identical.
//! 5. Write to `<path>.orchestra.tmp`.
//! 6. Rename to final path (atomic on POSIX).
//! 7. Update hash store entry + save store.

use std::path::{Path, PathBuf};

use chrono::Utc;
use sha2::{Digest, Sha256};

use orchestra_core::{
    registry,
    types::{Codebase, CodebaseName, ProjectName},
};
use orchestra_renderer::{AgentKind, Renderer, TemplateContext};

use crate::error::{io_err, SyncError};
use crate::hash_store;

// ---------------------------------------------------------------------------
// Write result
// ---------------------------------------------------------------------------

/// Outcome of an individual file write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteResult {
    /// File was written (content changed or did not previously exist).
    Written { path: PathBuf },
    /// File was skipped — rendered content matches the stored hash.
    Unchanged { path: PathBuf },
    /// `--dry-run` mode: the file *would* have been written.
    WouldWrite { path: PathBuf },
}

// ---------------------------------------------------------------------------
// atomic_write
// ---------------------------------------------------------------------------

/// Atomically write a single rendered file and update the hash store.
///
/// The hash store is loaded before the call; the caller is responsible for
/// saving it after all files for a codebase are processed.
///
/// Returns [`WriteResult`] indicating whether the file was written or skipped.
pub(crate) fn atomic_write(
    path: &Path,
    content: &str,
    hash_store: &mut hash_store::HashStore,
    dry_run: bool,
) -> Result<WriteResult, SyncError> {
    let tmp = PathBuf::from(format!("{}.orchestra.tmp", path.display()));
    atomic_write_with_tmp(path, content, hash_store, dry_run, &tmp)
}

fn atomic_write_with_tmp(
    path: &Path,
    content: &str,
    hash_store: &mut hash_store::HashStore,
    dry_run: bool,
    tmp: &Path,
) -> Result<WriteResult, SyncError> {
    // Normalise line endings to LF before hashing and writing.
    let normalized = content.replace("\r\n", "\n");
    let content = normalized.as_str();

    // Step 2: hash the normalised content.
    let digest = {
        let mut h = Sha256::new();
        h.update(content.as_bytes());
        hex::encode(h.finalize())
    };

    // Step 4: compare with stored hash.
    let key = path.to_string_lossy().to_string();
    if let Some(stored) = hash_store.get(&key) {
        if stored == &digest {
            tracing::debug!("unchanged: {}", path.display());
            return Ok(WriteResult::Unchanged {
                path: path.to_path_buf(),
            });
        }
    }

    if dry_run {
        tracing::info!("[dry-run] would write: {}", path.display());
        return Ok(WriteResult::WouldWrite {
            path: path.to_path_buf(),
        });
    }

    // Step 5: ensure parent directory exists, write to .tmp.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }

    if let Some(tmp_parent) = tmp.parent() {
        std::fs::create_dir_all(tmp_parent).map_err(|e| io_err(tmp_parent, e))?;
    }
    std::fs::write(tmp, content).map_err(|e| io_err(tmp, e))?;

    // Step 6: atomic rename to final path.
    if let Err(e) = std::fs::rename(tmp, path) {
        let _ = std::fs::remove_file(tmp);
        return Err(io_err(path, e));
    }

    // Step 7: update hash store entry (caller saves the store).
    hash_store.insert(key, digest);

    tracing::info!("wrote: {}", path.display());
    Ok(WriteResult::Written {
        path: path.to_path_buf(),
    })
}

// ---------------------------------------------------------------------------
// sync_codebase
// ---------------------------------------------------------------------------

pub(crate) fn build_sync_context(
    codebase: &Codebase,
    dry_run: bool,
    store_existed: bool,
    store_synced_at: chrono::DateTime<Utc>,
) -> TemplateContext {
    let mut ctx = TemplateContext::from_codebase(codebase);
    ctx.meta.last_synced = if dry_run {
        None
    } else if store_existed {
        Some(store_synced_at)
    } else {
        None
    };
    ctx
}

pub(crate) fn find_codebase_at(
    home: &Path,
    codebase_name: &str,
) -> Result<(ProjectName, Codebase), SyncError> {
    let name = CodebaseName::from(codebase_name);
    let all = registry::list_codebases_at(home)?;
    all.into_iter()
        .find(|(_, cb)| cb.name == name)
        .ok_or_else(|| {
            SyncError::Registry(orchestra_core::error::RegistryError::RegistryNotFound {
                path: home.join(".orchestra").join("projects").join(codebase_name),
            })
        })
}

/// Outcome of syncing a single codebase.
#[derive(Debug)]
pub struct SyncCodebaseResult {
    pub codebase_name: String,
    pub writes: Vec<WriteResult>,
}

/// Sync all agent files for the named codebase.
///
/// Renders every agent kind and writes with hash-gated atomic writes.
/// Returns a summary of what was written / unchanged.
pub fn sync_codebase(
    codebase_name: &str,
    home: &Path,
    dry_run: bool,
) -> Result<SyncCodebaseResult, SyncError> {
    let sync_started_at = Utc::now();

    // Find the codebase in the registry by scanning all projects.
    let (_, codebase) = find_codebase_at(home, codebase_name)?;

    let renderer = Renderer::new()?;
    let store_path = hash_store::store_path_at(home, codebase_name);
    let store_existed = store_path.exists();
    let mut store = hash_store::load_at(home, codebase_name)?;
    let ctx = build_sync_context(&codebase, dry_run, store_existed, store.synced_at);
    let mut writes = Vec::new();

    for agent in AgentKind::all() {
        let outputs = renderer.render_with_context(&ctx, *agent)?;
        for (path, content) in outputs {
            let result = atomic_write(&path, &content, &mut store.files, dry_run)?;
            writes.push(result);
        }
    }

    // Save the updated hash store (skip in dry-run — no filesystem changes).
    if !dry_run {
        store.synced_at = sync_started_at;
        hash_store::save_at(home, codebase_name, &store)?;
    }

    Ok(SyncCodebaseResult {
        codebase_name: codebase_name.to_string(),
        writes,
    })
}

// ---------------------------------------------------------------------------
// sync_all
// ---------------------------------------------------------------------------

/// Sync all registered codebases.
pub fn sync_all(home: &Path, dry_run: bool) -> Result<Vec<SyncCodebaseResult>, SyncError> {
    let all = registry::list_codebases_at(home)?;
    let mut results = Vec::new();
    for (_project, codebase) in all {
        let name = codebase.name.0.clone();
        let r = sync_codebase(&name, home, dry_run)?;
        results.push(r);
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    use orchestra_core::{
        registry,
        types::{Codebase, CodebaseName, Project, ProjectName, ProjectType},
    };
    use std::collections::HashMap;
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::TempDir;

    fn write_content(path: &Path, content: &str) -> WriteResult {
        let mut store = HashMap::new();
        atomic_write(path, content, &mut store, false).unwrap()
    }

    fn make_codebase_for_context(name: &str) -> Codebase {
        let now = Utc::now();
        Codebase {
            name: CodebaseName::from(name),
            path: PathBuf::from("/tmp").join(name),
            projects: vec![Project {
                name: ProjectName::from("api"),
                project_type: ProjectType::Backend,
                tasks: vec![],
                agents: vec![],
            }],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn first_write_returns_written() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        let result = write_content(&path, "hello");
        assert!(matches!(result, WriteResult::Written { .. }));
        assert!(path.exists());
    }

    #[test]
    fn second_write_same_content_returns_unchanged() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file.md");
        let mut store = HashMap::new();
        // First write.
        atomic_write(&path, "same content", &mut store, false).unwrap();
        // Second write with same content.
        let result = atomic_write(&path, "same content", &mut store, false).unwrap();
        assert!(matches!(result, WriteResult::Unchanged { .. }));
    }

    #[test]
    fn changed_content_returns_written() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file.md");
        let mut store = HashMap::new();
        atomic_write(&path, "v1", &mut store, false).unwrap();
        let result = atomic_write(&path, "v2", &mut store, false).unwrap();
        assert!(matches!(result, WriteResult::Written { .. }));
    }

    #[test]
    fn dry_run_does_not_write_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nope.md");
        let mut store = HashMap::new();
        let result = atomic_write(&path, "content", &mut store, true).unwrap();
        assert!(matches!(result, WriteResult::WouldWrite { .. }));
        assert!(!path.exists(), "dry-run must not create files");
    }

    #[test]
    fn tmp_file_removed_after_write() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("clean.md");
        write_content(&path, "data");
        let tmp_path = PathBuf::from(format!("{}.orchestra.tmp", path.display()));
        assert!(!tmp_path.exists(), ".orchestra.tmp must be cleaned up");
    }

    #[test]
    fn creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let path = tmp
            .path()
            .join(".cursor")
            .join("rules")
            .join("orchestra.mdc");
        write_content(&path, "content");
        assert!(path.exists());
    }

    #[test]
    fn hash_noop_preserves_mtime_and_hash() {
        let home = TempDir::new().unwrap();
        let codebase_root = TempDir::new().unwrap();
        let codebase_dir = codebase_root.path().join("copnow_api");
        fs::create_dir_all(&codebase_dir).unwrap();

        registry::init_at(
            codebase_dir.clone(),
            ProjectName::from("copnow"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");

        sync_codebase("copnow_api", home.path(), false).expect("first sync");

        let target = codebase_dir.join("CLAUDE.md");
        let mtime_1 = fs::metadata(&target).unwrap().modified().unwrap();
        let store_1 = hash_store::load_at(home.path(), "copnow_api").unwrap();
        let key = target.to_string_lossy().to_string();
        let hash_1 = store_1.files.get(&key).expect("hash entry").clone();

        sleep(Duration::from_millis(1100));
        sync_codebase("copnow_api", home.path(), false).expect("second sync");

        let mtime_2 = fs::metadata(&target).unwrap().modified().unwrap();
        let store_2 = hash_store::load_at(home.path(), "copnow_api").unwrap();
        let hash_2 = store_2.files.get(&key).expect("hash entry").clone();

        assert_eq!(mtime_2, mtime_1, "mtime changed; file was rewritten");
        assert_eq!(hash_2, hash_1, "hash entry changed on no-op");
    }

    #[test]
    fn synced_at_changes_only_after_real_sync() {
        let home = TempDir::new().unwrap();
        let codebase_root = TempDir::new().unwrap();
        let codebase_dir = codebase_root.path().join("copnow_api");
        fs::create_dir_all(&codebase_dir).unwrap();

        registry::init_at(
            codebase_dir.clone(),
            ProjectName::from("copnow"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");

        sync_codebase("copnow_api", home.path(), false).expect("first sync");
        let first = hash_store::load_at(home.path(), "copnow_api")
            .unwrap()
            .synced_at;

        sleep(Duration::from_millis(1100));
        sync_codebase("copnow_api", home.path(), true).expect("dry-run sync");
        let after_dry_run = hash_store::load_at(home.path(), "copnow_api")
            .unwrap()
            .synced_at;
        assert_eq!(after_dry_run, first, "dry-run must not advance synced_at");

        sleep(Duration::from_millis(1100));
        sync_codebase("copnow_api", home.path(), false).expect("second real sync");
        let second = hash_store::load_at(home.path(), "copnow_api")
            .unwrap()
            .synced_at;
        assert!(second > first, "real sync should advance synced_at");
    }

    #[test]
    fn crlf_and_lf_content_share_the_same_hash() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("normalize.md");
        let mut store = HashMap::new();

        let first = atomic_write(&path, "line1\r\nline2\r\n", &mut store, false).unwrap();
        assert!(matches!(first, WriteResult::Written { .. }));

        let second = atomic_write(&path, "line1\nline2\n", &mut store, false).unwrap();
        assert!(matches!(second, WriteResult::Unchanged { .. }));

        let disk = fs::read_to_string(&path).unwrap();
        assert_eq!(disk, "line1\nline2\n");
    }

    #[test]
    fn dry_run_context_has_no_last_synced() {
        let codebase = make_codebase_for_context("ctx");
        let synced_at = Utc::now();
        let ctx = build_sync_context(&codebase, true, true, synced_at);
        assert!(ctx.meta.last_synced.is_none());
    }

    #[test]
    fn non_dry_run_context_uses_store_synced_at_when_available() {
        let codebase = make_codebase_for_context("ctx");
        let synced_at = Utc::now() - ChronoDuration::hours(1);

        let with_store = build_sync_context(&codebase, false, true, synced_at);
        assert_eq!(with_store.meta.last_synced, Some(synced_at));

        let without_store = build_sync_context(&codebase, false, false, synced_at);
        assert!(without_store.meta.last_synced.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn rename_failure_leaves_original_and_cleans_tmp() {
        use std::os::unix::fs::PermissionsExt;

        let root = TempDir::new().unwrap();
        let readonly_dir = root.path().join("readonly");
        fs::create_dir_all(&readonly_dir).unwrap();

        let path = readonly_dir.join("file.md");
        fs::write(&path, "original").unwrap();

        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&readonly_dir, perms).unwrap();

        let tmp_dir = TempDir::new().unwrap();
        let tmp_path = tmp_dir.path().join("file.md.orchestra.tmp");

        let mut store = HashMap::new();
        let err = atomic_write_with_tmp(&path, "new content", &mut store, false, &tmp_path)
            .expect_err("rename should fail on readonly dir");
        let _ = err;

        let current = fs::read_to_string(&path).unwrap();
        assert_eq!(current, "original", "original file should be intact");
        assert!(!tmp_path.exists(), ".orchestra.tmp should be cleaned up");

        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&readonly_dir, perms).unwrap();
    }
}
