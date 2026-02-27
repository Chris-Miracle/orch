//! Domain types for the Orchestra registry.
//!
//! All path fields use `PathBuf`; never `&str` or `String` for filesystem paths.
//! All types are serializable/deserializable via serde + serde_yaml.

use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Newtypes
// ---------------------------------------------------------------------------

/// A strongly-typed name for a codebase entry in the registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodebaseName(pub String);

impl fmt::Display for CodebaseName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for CodebaseName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CodebaseName {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// A strongly-typed name for a project inside a codebase.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectName(pub String);

impl fmt::Display for ProjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for ProjectName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ProjectName {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// A strongly-typed identifier for an agent task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// The category of a codebase project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    #[default]
    Backend,
    Frontend,
    Mobile,
    Ml,
}

impl fmt::Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectType::Backend => write!(f, "backend"),
            ProjectType::Frontend => write!(f, "frontend"),
            ProjectType::Mobile => write!(f, "mobile"),
            ProjectType::Ml => write!(f, "ml"),
        }
    }
}

/// Status of a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Pending,
    InProgress,
    Blocked,
    Done,
}

// ---------------------------------------------------------------------------
// Domain structs
// ---------------------------------------------------------------------------

/// A single agent task within a codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Configuration for a specific AI agent assigned to a codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: String,
    /// Relative or absolute path to the agent's entry point file.
    pub entry_point: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
}

/// A project within a codebase (logical grouping of tasks and agents).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub name: ProjectName,
    pub project_type: ProjectType,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub agents: Vec<AgentConfig>,
}

/// A codebase managed by Orchestra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Codebase {
    pub name: CodebaseName,
    /// Absolute path to the codebase root on disk.
    pub path: PathBuf,
    #[serde(default)]
    pub projects: Vec<Project>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Root of the Orchestra YAML registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Registry {
    pub version: u32,
    #[serde(default)]
    pub codebases: Vec<Codebase>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newtype_display() {
        assert_eq!(CodebaseName::from("foo").to_string(), "foo");
        assert_eq!(ProjectName::from("bar").to_string(), "bar");
        assert_eq!(TaskId::from("t-01").to_string(), "t-01");
    }

    #[test]
    fn newtype_equality() {
        let a = CodebaseName::from("x");
        let b = CodebaseName::from(String::from("x"));
        assert_eq!(a, b);
    }

    #[test]
    fn registry_serde_roundtrip() {
        let now = Utc::now();
        let reg = Registry {
            version: 1,
            codebases: vec![],
            created_at: now,
            updated_at: now,
        };
        let yaml = serde_yaml::to_string(&reg).expect("serialize");
        let deserialized: Registry = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(reg.version, deserialized.version);
        assert_eq!(reg.codebases, deserialized.codebases);
    }

    #[test]
    fn project_type_display() {
        assert_eq!(ProjectType::Mobile.to_string(), "mobile");
        assert_eq!(ProjectType::Ml.to_string(), "ml");
    }
}
