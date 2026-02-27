//! Roundtrip serialisation tests for `orchestra-core` types.
//!
//! Each `#[case]` is isolated — no shared state.

use chrono::Utc;
use orchestra_core::types::{
    AgentConfig, Codebase, CodebaseName, Project, ProjectName, ProjectType, Registry, Task,
    TaskId, TaskStatus,
};
use rstest::rstest;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minimal_registry() -> Registry {
    let now = Utc::now();
    Registry { version: 1, codebases: vec![], created_at: now, updated_at: now }
}

fn full_registry() -> Registry {
    let now = Utc::now();
    Registry {
        version: 1,
        codebases: vec![Codebase {
            name: CodebaseName::from("my-app"),
            path: PathBuf::from("/code/my-app"),
            projects: vec![Project {
                name: ProjectName::from("api"),
                project_type: ProjectType::Backend,
                tasks: vec![Task {
                    id: TaskId::from("t-001"),
                    title: "Implement auth".to_string(),
                    status: TaskStatus::InProgress,
                    description: Some("JWT-based auth flow".to_string()),
                    created_at: now,
                    updated_at: now,
                }],
                agents: vec![AgentConfig {
                    agent_id: "claude".to_string(),
                    entry_point: PathBuf::from("AGENT/CLAUDE.md"),
                    skills: Some(vec!["registry-foundation".to_string()]),
                }],
            }],
            created_at: now,
            updated_at: now,
        }],
        created_at: now,
        updated_at: now,
    }
}

fn unicode_registry() -> Registry {
    let now = Utc::now();
    Registry {
        version: 1,
        codebases: vec![Codebase {
            name: CodebaseName::from("アプリ-проект-项目"),
            path: PathBuf::from("/code/unicode-app"),
            projects: vec![Project {
                name: ProjectName::from("пользователь-api"),
                project_type: ProjectType::Backend,
                tasks: vec![Task {
                    id: TaskId::from("t-🚀"),
                    title: "Task with émojis & spéçïal chars: <>&\"'".to_string(),
                    status: TaskStatus::Pending,
                    description: Some("日本語・한국어・العربية".to_string()),
                    created_at: now,
                    updated_at: now,
                }],
                agents: vec![],
            }],
            created_at: now,
            updated_at: now,
        }],
        created_at: now,
        updated_at: now,
    }
}

fn empty_vecs_registry() -> Registry {
    let now = Utc::now();
    Registry {
        version: 1,
        codebases: vec![Codebase {
            name: CodebaseName::from("empty"),
            path: PathBuf::from("/code/empty"),
            projects: vec![],
            created_at: now,
            updated_at: now,
        }],
        created_at: now,
        updated_at: now,
    }
}

// ---------------------------------------------------------------------------
// Parameterised roundtrip test
// ---------------------------------------------------------------------------

#[rstest]
#[case("minimal", minimal_registry())]
#[case("all_fields", full_registry())]
#[case("unicode_strings", unicode_registry())]
#[case("empty_vecs", empty_vecs_registry())]
fn registry_roundtrip(#[case] label: &str, #[case] registry: Registry) {
    let yaml = serde_yaml::to_string(&registry)
        .unwrap_or_else(|e| panic!("[{label}] serialize failed: {e}"));
    let back: Registry = serde_yaml::from_str(&yaml)
        .unwrap_or_else(|e| panic!("[{label}] deserialize failed: {e}"));
    assert_eq!(registry.version, back.version, "[{label}] version");
    assert_eq!(registry.codebases.len(), back.codebases.len(), "[{label}] codebase count");
    for (orig, got) in registry.codebases.iter().zip(back.codebases.iter()) {
        assert_eq!(orig.name, got.name, "[{label}] codebase name");
        assert_eq!(orig.path, got.path, "[{label}] codebase path");
        assert_eq!(orig.projects.len(), got.projects.len(), "[{label}] project count");
        for (op, gp) in orig.projects.iter().zip(got.projects.iter()) {
            assert_eq!(op.name, gp.name, "[{label}] project name");
            assert_eq!(op.project_type, gp.project_type, "[{label}] project type");
            for (ot, gt) in op.tasks.iter().zip(gp.tasks.iter()) {
                assert_eq!(ot.id, gt.id, "[{label}] task id");
                assert_eq!(ot.title, gt.title, "[{label}] task title");
                assert_eq!(ot.description, gt.description, "[{label}] task description");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Codebase-only roundtrip (all ProjectType variants)
// ---------------------------------------------------------------------------

#[rstest]
#[case(ProjectType::Backend)]
#[case(ProjectType::Frontend)]
#[case(ProjectType::Mobile)]
#[case(ProjectType::Ml)]
fn project_type_roundtrip(#[case] pt: ProjectType) {
    let project = Project {
        name: ProjectName::from("test"),
        project_type: pt,
        tasks: vec![],
        agents: vec![],
    };
    let yaml = serde_yaml::to_string(&project).expect("serialize");
    let back: Project = serde_yaml::from_str(&yaml).expect("deserialize");
    assert_eq!(project.project_type, back.project_type);
}
