//! Shared sync pipeline entrypoint used by CLI and daemon.

use std::path::Path;

use crate::{sync_all, sync_codebase, SyncCodebaseResult, SyncError};

/// Scope for a sync pipeline run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncScope {
    /// Sync every registered codebase.
    All,
    /// Sync a single named codebase.
    Codebase(String),
}

/// Run the sync pipeline for a scope.
///
/// This is the canonical sync entrypoint for both `orchestra sync` and the
/// Phase 04 daemon processor.
pub fn run(
    home: &Path,
    scope: SyncScope,
    dry_run: bool,
) -> Result<Vec<SyncCodebaseResult>, SyncError> {
    match scope {
        SyncScope::All => sync_all(home, dry_run),
        SyncScope::Codebase(name) => Ok(vec![sync_codebase(&name, home, dry_run)?]),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use orchestra_core::{
        registry,
        types::{ProjectName, ProjectType},
    };
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn run_all_empty_registry_returns_empty_vec() {
        let home = TempDir::new().expect("home");
        let result = run(home.path(), SyncScope::All, true).expect("run");
        assert!(result.is_empty());
    }

    #[test]
    fn run_single_codebase_returns_single_result() {
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

        let result = run(
            home.path(),
            SyncScope::Codebase("copnow_api".to_string()),
            true,
        )
        .expect("run");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].codebase_name, "copnow_api");
    }
}
