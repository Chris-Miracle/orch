//! Registry load/save and init logic.
//!
//! **Core API** (`_at` variants): accept an explicit `home: &Path` — used in tests
//! with `TempDir` so that no test ever touches the real `~/.orchestra`.
//!
//! **Convenience wrappers**: `load()`, `save()`, `init()`, `add_project()` derive
//! `home` from `dirs::home_dir()` and delegate to the `_at` variants.

use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::error::RegistryError;
use crate::types::{Codebase, CodebaseName, Project, ProjectName, ProjectType, Registry};

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns `<home>/.orchestra/registry.yaml` without touching the filesystem.
pub fn registry_path_at(home: &Path) -> PathBuf {
    home.join(".orchestra").join("registry.yaml")
}

/// Returns `~/.orchestra/registry.yaml`.
pub fn registry_path() -> Result<PathBuf, RegistryError> {
    Ok(registry_path_at(&home()?))
}

/// Ensures `<home>/.orchestra/` exists with mode `0700` and returns its path.
pub fn registry_dir_at(home: &Path) -> Result<PathBuf, RegistryError> {
    let dir = home.join(".orchestra");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
        set_dir_permissions(&dir)?;
    }
    Ok(dir)
}

/// Ensures `~/.orchestra/` exists and returns its path.
pub fn registry_dir() -> Result<PathBuf, RegistryError> {
    registry_dir_at(&home()?)
}

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

/// Load the registry from `<home>/.orchestra/registry.yaml`.
///
/// Returns `RegistryError::RegistryNotFound` if absent,
/// `RegistryError::Parse` (with path + line context) if malformed.
pub fn load_at(home: &Path) -> Result<Registry, RegistryError> {
    let path = registry_path_at(home);
    if !path.exists() {
        return Err(RegistryError::RegistryNotFound { path });
    }
    let contents = std::fs::read_to_string(&path)?;
    let registry: Registry = serde_yaml::from_str(&contents)
        .map_err(|e| RegistryError::Parse { path, source: e })?;
    Ok(registry)
}

/// Load the registry from `~/.orchestra/registry.yaml`.
pub fn load() -> Result<Registry, RegistryError> {
    load_at(&home()?)
}

// ---------------------------------------------------------------------------
// Save (atomic)
// ---------------------------------------------------------------------------

/// Atomically save the registry under `<home>/.orchestra/registry.yaml`.
///
/// Writes to a `.tmp` sibling (same directory = same filesystem on macOS),
/// sets `0600` permissions, then renames atomically.
pub fn save_at(registry: &Registry, home: &Path) -> Result<(), RegistryError> {
    let path = registry_path_at(home);
    registry_dir_at(home)?; // ensure dir + perms

    let tmp_path = path.with_file_name("registry.yaml.tmp");
    let yaml = serde_yaml::to_string(registry)?;
    std::fs::write(&tmp_path, yaml)?;
    set_file_permissions(&tmp_path)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Atomically save the registry to `~/.orchestra/registry.yaml`.
pub fn save(registry: &Registry) -> Result<(), RegistryError> {
    save_at(registry, &home()?)
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

/// Initialise (or update) the registry for a codebase at `codebase_path`.
///
/// If a registry already exists at `<home>/.orchestra/registry.yaml`, the new
/// codebase is appended (if not already tracked). Saved atomically.
pub fn init_at(
    codebase_path: PathBuf,
    project_name: ProjectName,
    project_type: Option<ProjectType>,
    home: &Path,
) -> Result<Registry, RegistryError> {
    let now = Utc::now();
    let mut registry = match load_at(home) {
        Ok(r) => r,
        Err(RegistryError::RegistryNotFound { .. }) => Registry {
            version: 1,
            codebases: vec![],
            created_at: now,
            updated_at: now,
        },
        Err(e) => return Err(e),
    };

    let codebase_name = CodebaseName::from(
        codebase_path
            .file_name()
            .unwrap_or_else(|| codebase_path.as_os_str())
            .to_string_lossy()
            .into_owned(),
    );

    let already_tracked = registry.codebases.iter().any(|c| c.path == codebase_path);
    if !already_tracked {
        registry.codebases.push(Codebase {
            name: codebase_name,
            path: codebase_path,
            projects: vec![Project {
                name: project_name,
                project_type: project_type.unwrap_or_default(),
                tasks: vec![],
                agents: vec![],
            }],
            created_at: now,
            updated_at: now,
        });
        registry.updated_at = now;
    }

    save_at(&registry, home)?;
    Ok(registry)
}

/// Initialise the registry using `~/.orchestra` as home.
pub fn init(
    codebase_path: PathBuf,
    project_name: ProjectName,
    project_type: Option<ProjectType>,
) -> Result<Registry, RegistryError> {
    init_at(codebase_path, project_name, project_type, &home()?)
}

// ---------------------------------------------------------------------------
// Project management
// ---------------------------------------------------------------------------

/// Add a project to the first codebase; save atomically under `home`.
pub fn add_project_at(
    project_name: ProjectName,
    project_type: ProjectType,
    home: &Path,
) -> Result<Registry, RegistryError> {
    let mut registry = load_at(home)?;
    let now = Utc::now();
    if let Some(codebase) = registry.codebases.first_mut() {
        if !codebase.projects.iter().any(|p| p.name == project_name) {
            codebase.projects.push(Project {
                name: project_name,
                project_type,
                tasks: vec![],
                agents: vec![],
            });
            codebase.updated_at = now;
        }
    }
    registry.updated_at = now;
    save_at(&registry, home)?;
    Ok(registry)
}

/// Add a project to the first registered codebase using `~/.orchestra` as home.
pub fn add_project(
    project_name: ProjectName,
    project_type: ProjectType,
) -> Result<Registry, RegistryError> {
    add_project_at(project_name, project_type, &home()?)
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
// Unit tests (home-independent)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn roundtrip(reg: &Registry) -> Registry {
        let yaml = serde_yaml::to_string(reg).expect("serialize");
        serde_yaml::from_str(&yaml).expect("deserialize")
    }

    #[test]
    fn empty_registry_roundtrip() {
        let now = Utc::now();
        let reg = Registry { version: 1, codebases: vec![], created_at: now, updated_at: now };
        let back = roundtrip(&reg);
        assert_eq!(reg.version, back.version);
        assert!(back.codebases.is_empty());
    }

    #[test]
    fn codebase_with_project_roundtrip() {
        use crate::types::{Codebase, CodebaseName, Project, ProjectName, ProjectType};
        let now = Utc::now();
        let reg = Registry {
            version: 1,
            codebases: vec![Codebase {
                name: CodebaseName::from("myapp"),
                path: PathBuf::from("/code/myapp"),
                projects: vec![Project {
                    name: ProjectName::from("api"),
                    project_type: ProjectType::Backend,
                    tasks: vec![],
                    agents: vec![],
                }],
                created_at: now,
                updated_at: now,
            }],
            created_at: now,
            updated_at: now,
        };
        let back = roundtrip(&reg);
        assert_eq!(back.codebases[0].name, CodebaseName::from("myapp"));
        assert_eq!(back.codebases[0].projects[0].name, ProjectName::from("api"));
    }

    #[test]
    fn registry_not_found_error() {
        let dir = TempDir::new().expect("tempdir");
        let err = load_at(dir.path()).unwrap_err();
        assert!(matches!(err, RegistryError::RegistryNotFound { .. }));
        assert!(err.to_string().contains("registry not found"));
    }

    #[test]
    fn home_not_found_error_message() {
        assert!(RegistryError::HomeNotFound.to_string().contains("home directory"));
    }

    #[test]
    fn atomic_write_cleans_up_tmp() {
        let home = TempDir::new().expect("tempdir");
        let now = Utc::now();
        let reg = Registry { version: 1, codebases: vec![], created_at: now, updated_at: now };
        save_at(&reg, home.path()).expect("save");
        let tmp = registry_path_at(home.path()).with_file_name("registry.yaml.tmp");
        assert!(!tmp.exists(), ".tmp must be removed after successful save");
    }
}
