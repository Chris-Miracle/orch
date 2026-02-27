//! Per-codebase YAML registry.
//!
//! # Storage layout
//!
//! ```text
//! ~/.orchestra/
//!   projects/
//!     <project_name>/
//!       project.yaml          (index — mode 0600, created on first init)
//!       <codebase_name>.yaml  (one file per codebase — mode 0600)
//! ```
//!
//! # API pattern
//!
//! Every mutating function has two forms:
//! - `fn_at(home: &Path, …)` — explicit home; used in tests with `TempDir`
//! - `fn(…)` — derives home from `dirs::home_dir()`, delegates to `_at`
//!
//! Tests must NEVER call the no-arg wrappers; always use `_at`.

use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::error::RegistryError;
use crate::types::{Codebase, CodebaseName, Project, ProjectName, ProjectType};

// ---------------------------------------------------------------------------
// 1. Path helpers
// ---------------------------------------------------------------------------

/// `<home>/.orchestra/projects/<project>/`
///
/// Creates the directory (mode `0700`) if it does not yet exist.
pub fn project_dir_at(home: &Path, project: &ProjectName) -> Result<PathBuf, RegistryError> {
    let dir = home.join(".orchestra").join("projects").join(&project.0);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
        set_dir_permissions(&dir)?;
    }
    Ok(dir)
}

/// `<home>/.orchestra/projects/<project>/` (convenience — uses `dirs::home_dir()`).
pub fn project_dir(project: &ProjectName) -> Result<PathBuf, RegistryError> {
    project_dir_at(&home()?, project)
}

/// `<home>/.orchestra/projects/<project>/<codebase>.yaml` — pure, no I/O.
pub fn codebase_path_at(
    home: &Path,
    project: &ProjectName,
    codebase: &CodebaseName,
) -> PathBuf {
    home.join(".orchestra")
        .join("projects")
        .join(&project.0)
        .join(format!("{}.yaml", codebase.0))
}

/// Lists the names of all project directories under `<home>/.orchestra/projects/`.
pub fn list_project_names_at(home: &Path) -> Result<Vec<ProjectName>, RegistryError> {
    let dir = home.join(".orchestra").join("projects");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names: Vec<ProjectName> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| ProjectName::from(e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(names)
}

/// `list_project_names_at` convenience wrapper.
pub fn list_project_names() -> Result<Vec<ProjectName>, RegistryError> {
    list_project_names_at(&home()?)
}

// ---------------------------------------------------------------------------
// 2. Load
// ---------------------------------------------------------------------------

/// Load a single codebase from `<home>/.orchestra/projects/<project>/<codebase>.yaml`.
///
/// Returns `RegistryError::RegistryNotFound` if absent,
/// `RegistryError::Parse` (with path + line context) if malformed YAML.
pub fn load_codebase_at(
    home: &Path,
    project: &ProjectName,
    codebase: &CodebaseName,
) -> Result<Codebase, RegistryError> {
    let path = codebase_path_at(home, project, codebase);
    if !path.exists() {
        return Err(RegistryError::RegistryNotFound { path });
    }
    let contents = std::fs::read_to_string(&path)?;
    serde_yaml::from_str(&contents).map_err(|e| RegistryError::Parse { path, source: e })
}

/// `load_codebase_at` convenience wrapper.
pub fn load_codebase(
    project: &ProjectName,
    codebase: &CodebaseName,
) -> Result<Codebase, RegistryError> {
    load_codebase_at(&home()?, project, codebase)
}

/// Walk `<home>/.orchestra/projects/*/*.yaml` and return all codebases grouped
/// by project. Results are sorted deterministically (project name, then codebase name).
///
/// Skips `project.yaml` index files.
pub fn list_codebases_at(
    home: &Path,
) -> Result<Vec<(ProjectName, Codebase)>, RegistryError> {
    let projects_dir = home.join(".orchestra").join("projects");
    if !projects_dir.exists() {
        return Ok(vec![]);
    }

    let mut project_entries: Vec<_> = std::fs::read_dir(&projects_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    project_entries.sort_by_key(|e| e.file_name());

    let mut result = Vec::new();
    for proj_entry in project_entries {
        let project_name = ProjectName::from(proj_entry.file_name().to_string_lossy().into_owned());

        let mut file_entries: Vec<_> = std::fs::read_dir(proj_entry.path())?
            .filter_map(|e| e.ok())
            .collect();
        file_entries.sort_by_key(|e| e.file_name());

        for file_entry in file_entries {
            let fname = file_entry.file_name();
            let name = fname.to_string_lossy();
            if !name.ends_with(".yaml") || name == "project.yaml" {
                continue;
            }
            let contents = std::fs::read_to_string(file_entry.path())?;
            let codebase: Codebase = serde_yaml::from_str(&contents).map_err(|e| {
                RegistryError::Parse { path: file_entry.path(), source: e }
            })?;
            result.push((project_name.clone(), codebase));
        }
    }
    Ok(result)
}

/// `list_codebases_at` convenience wrapper.
pub fn list_codebases() -> Result<Vec<(ProjectName, Codebase)>, RegistryError> {
    list_codebases_at(&home()?)
}

// ---------------------------------------------------------------------------
// 3. Save (atomic)
// ---------------------------------------------------------------------------

/// Atomically save a codebase to `<home>/.orchestra/projects/<project>/<codebase>.yaml`.
///
/// Write flow: serialize → `.yaml.tmp` sibling → `chmod 0600` → `rename`.
/// `.tmp` is always in the same directory as the target (same filesystem — no EXDEV on macOS).
pub fn save_codebase_at(
    home: &Path,
    project: &ProjectName,
    codebase: &Codebase,
) -> Result<(), RegistryError> {
    project_dir_at(home, project)?; // create dir + 0700 if absent
    let path = codebase_path_at(home, project, &codebase.name);
    let tmp_path = path.with_file_name(format!("{}.yaml.tmp", codebase.name.0));

    let yaml = serde_yaml::to_string(codebase)?;
    std::fs::write(&tmp_path, yaml)?;
    set_file_permissions(&tmp_path)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// `save_codebase_at` convenience wrapper.
pub fn save_codebase(
    project: &ProjectName,
    codebase: &Codebase,
) -> Result<(), RegistryError> {
    save_codebase_at(&home()?, project, codebase)
}

// ---------------------------------------------------------------------------
// 4. Project index (optional scaffold)
// ---------------------------------------------------------------------------

/// Write `<home>/.orchestra/projects/<project>/project.yaml` if it doesn't exist.
fn scaffold_project_index(home: &Path, project: &ProjectName) -> Result<(), RegistryError> {
    let dir = project_dir_at(home, project)?;
    let index_path = dir.join("project.yaml");
    if index_path.exists() {
        return Ok(());
    }
    let content = format!("name: {}\ncreated_at: {}\n", project.0, Utc::now().to_rfc3339());
    let tmp = dir.join("project.yaml.tmp");
    std::fs::write(&tmp, content)?;
    set_file_permissions(&tmp)?;
    std::fs::rename(&tmp, &index_path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Init
// ---------------------------------------------------------------------------

/// Register a codebase at `codebase_path` under `project_name`.
///
/// Creates `<home>/.orchestra/projects/<project_name>/<codebase_name>.yaml`.
/// Idempotent: if the file already exists, loads and returns it unchanged.
pub fn init_at(
    codebase_path: PathBuf,
    project_name: ProjectName,
    project_type: Option<ProjectType>,
    home: &Path,
) -> Result<Codebase, RegistryError> {
    let now = Utc::now();
    let codebase_name = CodebaseName::from(
        codebase_path
            .file_name()
            .unwrap_or_else(|| codebase_path.as_os_str())
            .to_string_lossy()
            .into_owned(),
    );

    // Idempotent: return existing if already registered
    let yaml_path = codebase_path_at(home, &project_name, &codebase_name);
    if yaml_path.exists() {
        return load_codebase_at(home, &project_name, &codebase_name);
    }

    let codebase = Codebase {
        name: codebase_name.clone(),
        path: codebase_path,
        projects: vec![Project {
            name: ProjectName::from(codebase_name.0.clone()),
            project_type: project_type.unwrap_or_default(),
            tasks: vec![],
            agents: vec![],
        }],
        created_at: now,
        updated_at: now,
    };

    scaffold_project_index(home, &project_name)?;
    save_codebase_at(home, &project_name, &codebase)?;
    Ok(codebase)
}

/// `init_at` convenience wrapper.
pub fn init(
    codebase_path: PathBuf,
    project_name: ProjectName,
    project_type: Option<ProjectType>,
) -> Result<Codebase, RegistryError> {
    init_at(codebase_path, project_name, project_type, &home()?)
}

// ---------------------------------------------------------------------------
// 6. Add codebase
// ---------------------------------------------------------------------------

/// Register a new named codebase inside an existing project directory.
///
/// Creates `<home>/.orchestra/projects/<project>/<codebase_name>.yaml`.
/// Returns `RegistryError::RegistryNotFound` if the project directory doesn't exist.
/// Idempotent: returns the existing file if already present.
pub fn add_codebase_at(
    home: &Path,
    project: &ProjectName,
    codebase_name: CodebaseName,
    project_type: ProjectType,
) -> Result<Codebase, RegistryError> {
    let project_dir = home.join(".orchestra").join("projects").join(&project.0);
    if !project_dir.exists() {
        return Err(RegistryError::RegistryNotFound { path: project_dir });
    }

    let yaml_path = codebase_path_at(home, project, &codebase_name);
    if yaml_path.exists() {
        return load_codebase_at(home, project, &codebase_name);
    }

    let now = Utc::now();
    let codebase = Codebase {
        name: codebase_name.clone(),
        path: PathBuf::from(&codebase_name.0),
        projects: vec![Project {
            name: ProjectName::from(codebase_name.0.clone()),
            project_type,
            tasks: vec![],
            agents: vec![],
        }],
        created_at: now,
        updated_at: now,
    };

    save_codebase_at(home, project, &codebase)?;
    Ok(codebase)
}

/// `add_codebase_at` convenience wrapper.
pub fn add_codebase(
    project: &ProjectName,
    codebase_name: CodebaseName,
    project_type: ProjectType,
) -> Result<Codebase, RegistryError> {
    add_codebase_at(&home()?, project, codebase_name, project_type)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn home() -> Result<PathBuf, RegistryError> {
    dirs::home_dir().ok_or(RegistryError::HomeNotFound)
}

#[cfg(unix)]
fn set_dir_permissions(path: &Path) -> Result<(), RegistryError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}
#[cfg(not(unix))]
fn set_dir_permissions(_path: &Path) -> Result<(), RegistryError> {
    Ok(())
}

#[cfg(unix)]
fn set_file_permissions(path: &Path) -> Result<(), RegistryError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}
#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> Result<(), RegistryError> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_home() -> TempDir {
        TempDir::new().expect("tempdir")
    }

    fn proj() -> ProjectName {
        ProjectName::from("copnow")
    }
    fn cb_name() -> CodebaseName {
        CodebaseName::from("copnow_api")
    }

    #[test]
    fn codebase_path_is_correct() {
        let home = make_home();
        let path = codebase_path_at(home.path(), &proj(), &cb_name());
        assert!(path.ends_with(".orchestra/projects/copnow/copnow_api.yaml"));
    }

    #[test]
    fn project_dir_created_with_perms() {
        let home = make_home();
        let dir = project_dir_at(home.path(), &proj()).expect("project_dir_at");
        assert!(dir.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
    }

    #[test]
    fn save_and_load_codebase_roundtrip() {
        let home = make_home();
        let now = Utc::now();
        let cb = Codebase {
            name: cb_name(),
            path: PathBuf::from("/code/copnow_api"),
            projects: vec![Project {
                name: ProjectName::from("api"),
                project_type: ProjectType::Backend,
                tasks: vec![],
                agents: vec![],
            }],
            created_at: now,
            updated_at: now,
        };
        save_codebase_at(home.path(), &proj(), &cb).expect("save");
        let loaded = load_codebase_at(home.path(), &proj(), &cb_name()).expect("load");
        assert_eq!(loaded.name, cb.name);
        assert_eq!(loaded.path, cb.path);
    }

    #[test]
    fn atomic_write_cleans_up_tmp() {
        let home = make_home();
        let now = Utc::now();
        let cb = Codebase {
            name: cb_name(),
            path: PathBuf::from("/code/x"),
            projects: vec![],
            created_at: now,
            updated_at: now,
        };
        save_codebase_at(home.path(), &proj(), &cb).expect("save");
        let tmp = codebase_path_at(home.path(), &proj(), &cb_name())
            .with_file_name("copnow_api.yaml.tmp");
        assert!(!tmp.exists(), ".tmp must be gone after successful save");
    }

    #[test]
    fn load_missing_codebase_returns_not_found() {
        let home = make_home();
        let err = load_codebase_at(home.path(), &proj(), &cb_name()).unwrap_err();
        assert!(matches!(err, RegistryError::RegistryNotFound { .. }));
    }

    #[test]
    fn list_codebases_empty_when_no_projects() {
        let home = make_home();
        let list = list_codebases_at(home.path()).expect("list");
        assert!(list.is_empty());
    }

    #[test]
    fn home_not_found_error_message() {
        assert!(RegistryError::HomeNotFound.to_string().contains("home directory"));
    }
}
