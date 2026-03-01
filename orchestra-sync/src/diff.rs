//! Dry-run unified diff support for `orchestra diff`.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use similar::TextDiff;

use orchestra_renderer::{AgentKind, Renderer};

use crate::{
    error::io_err,
    hash_store,
    writer::{build_sync_context, find_codebase_at},
    SyncError,
};

/// A single rendered file diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: PathBuf,
    pub unified_diff: String,
}

/// Diff result for a codebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffCodebaseResult {
    pub codebase_name: String,
    pub diffs: Vec<FileDiff>,
}

/// Render what `sync` would generate and compare it to current on-disk content.
///
/// No files are written.
pub fn diff_codebase(codebase_name: &str, home: &Path) -> Result<DiffCodebaseResult, SyncError> {
    let (_project, codebase) = find_codebase_at(home, codebase_name)?;
    let renderer = Renderer::new()?;

    let store_path = hash_store::store_path_at(home, codebase_name);
    let store_existed = store_path.exists();
    let store = hash_store::load_at(home, codebase_name)?;
    let mut ctx = build_sync_context(&codebase, false, store_existed, store.synced_at);
    ctx.meta.last_synced = None;

    let mut diffs = Vec::new();
    for agent in AgentKind::all() {
        let outputs = renderer.render_with_context(&ctx, *agent)?;
        for (path, rendered) in outputs {
            let rendered = normalize_line_endings(&rendered);
            let existing = read_existing_or_empty(&path)?;
            if existing == rendered {
                continue;
            }

            let relative = path.strip_prefix(&codebase.path).unwrap_or(path.as_path());
            let old_header = format!("a/{}", relative.display());
            let new_header = format!("b/{}", relative.display());
            let unified = TextDiff::from_lines(&existing, &rendered)
                .unified_diff()
                .header(&old_header, &new_header)
                .context_radius(3)
                .to_string();

            diffs.push(FileDiff {
                path,
                unified_diff: unified,
            });
        }
    }

    Ok(DiffCodebaseResult {
        codebase_name: codebase_name.to_string(),
        diffs,
    })
}

fn read_existing_or_empty(path: &Path) -> Result<String, SyncError> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(normalize_line_endings(&content)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(io_err(path, err)),
    }
}

fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::Duration as ChronoDuration;
    use orchestra_core::{
        registry,
        types::{ProjectName, ProjectType},
    };
    use tempfile::TempDir;

    use crate::{hash_store, sync_codebase};

    use super::*;

    #[test]
    fn no_diffs_after_clean_sync() {
        let home = TempDir::new().expect("home");
        let workspace = TempDir::new().expect("workspace");
        let codebase_dir = workspace.path().join("copnow_api");
        fs::create_dir_all(&codebase_dir).expect("mkdir");
        registry::init_at(
            codebase_dir,
            ProjectName::from("copnow"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");
        sync_codebase("copnow_api", home.path(), false).expect("sync");

        let diff = diff_codebase("copnow_api", home.path()).expect("diff");
        assert!(diff.diffs.is_empty(), "synced codebase should have no diff");
    }

    #[test]
    fn local_edit_produces_unified_diff() {
        let home = TempDir::new().expect("home");
        let workspace = TempDir::new().expect("workspace");
        let codebase_dir = workspace.path().join("copnow_api");
        fs::create_dir_all(&codebase_dir).expect("mkdir");
        registry::init_at(
            codebase_dir.clone(),
            ProjectName::from("copnow"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");
        sync_codebase("copnow_api", home.path(), false).expect("sync");

        let target = codebase_dir.join("CLAUDE.md");
        let edited = format!(
            "{}\nmanual tweak\n",
            fs::read_to_string(&target).expect("read")
        );
        fs::write(&target, edited).expect("write");

        let diff = diff_codebase("copnow_api", home.path()).expect("diff");
        assert!(!diff.diffs.is_empty(), "expected at least one file diff");

        let claude_diff = diff
            .diffs
            .iter()
            .find(|d| d.path.ends_with("CLAUDE.md"))
            .expect("CLAUDE diff");
        assert!(claude_diff.unified_diff.contains("--- a/CLAUDE.md"));
        assert!(claude_diff.unified_diff.contains("+++ b/CLAUDE.md"));
        assert!(claude_diff.unified_diff.contains("@@"));
    }

    #[test]
    fn synced_at_changes_do_not_create_diff_noise() {
        let home = TempDir::new().expect("home");
        let workspace = TempDir::new().expect("workspace");
        let codebase_dir = workspace.path().join("copnow_api");
        fs::create_dir_all(&codebase_dir).expect("mkdir");
        registry::init_at(
            codebase_dir,
            ProjectName::from("copnow"),
            Some(ProjectType::Backend),
            home.path(),
        )
        .expect("init");
        sync_codebase("copnow_api", home.path(), false).expect("sync");

        let mut store = hash_store::load_at(home.path(), "copnow_api").expect("store");
        store.synced_at += ChronoDuration::hours(2);
        hash_store::save_at(home.path(), "copnow_api", &store).expect("save");

        let diff = diff_codebase("copnow_api", home.path()).expect("diff");
        assert!(
            diff.diffs.is_empty(),
            "last_synced metadata changes must not produce diff output"
        );
    }
}
