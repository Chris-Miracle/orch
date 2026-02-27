//! Template context — serializable rendering payload built from [`Codebase`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use orchestra_core::types::{Codebase, TaskStatus};

use crate::error::RenderError;

/// Flat + structured rendering payload.
///
/// The FRD-aligned nested shape is exposed via `identity`, `stack`,
/// `commands`, `architecture`, `skills`, `tasks`, `subagents`, and `meta`.
/// Legacy flat fields are retained so existing templates keep working.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateContext {
    /// FRD identity context.
    pub identity: IdentityCtx,
    /// FRD stack context.
    pub stack: StackCtx,
    /// FRD command context.
    pub commands: CommandsCtx,
    /// FRD architecture context.
    pub architecture: ArchitectureCtx,
    /// Additional conventions to include.
    pub conventions: Vec<String>,
    /// FRD skill entries.
    pub skills: Vec<SkillCtx>,
    /// FRD task entries (done tasks excluded).
    pub tasks: Vec<TaskCtx>,
    /// FRD subagent entries.
    pub subagents: Vec<SubagentCtx>,
    /// FRD meta info.
    pub meta: MetaCtx,

    /// Legacy field kept for backward-compatible templates.
    pub codebase_name: String,
    /// Legacy field kept for backward-compatible templates.
    pub codebase_path: String,
    /// Legacy field kept for backward-compatible templates.
    pub projects: Vec<ProjectSummary>,
    /// Legacy field kept for backward-compatible templates.
    pub active_task_count: usize,
}

/// FRD identity context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCtx {
    pub codebase_name: String,
    pub codebase_path: String,
}

/// FRD stack context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackCtx {
    pub projects: Vec<ProjectSummary>,
}

/// FRD commands context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsCtx {
    pub sync: String,
    pub sync_dry_run: String,
}

/// FRD architecture context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureCtx {
    pub summary: String,
}

/// FRD skill context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCtx {
    pub id: String,
    pub description: String,
}

/// FRD task context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCtx {
    pub id: String,
    pub title: String,
    pub status: String,
    pub description: Option<String>,
}

/// FRD subagent context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentCtx {
    pub id: String,
    pub entry_point: String,
    pub skills: Vec<String>,
}

/// FRD meta context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaCtx {
    pub orchestra_version: String,
    pub last_synced: Option<DateTime<Utc>>,
}

/// Serializable summary of a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub name: String,
    pub project_type: String,
}

/// Backward-compatible alias used by existing tests/imports.
pub type TaskSummary = TaskCtx;

impl TemplateContext {
    /// Build a [`TemplateContext`] from a [`Codebase`].
    pub fn from_codebase(codebase: &Codebase) -> Self {
        let projects: Vec<ProjectSummary> = codebase
            .projects
            .iter()
            .map(|p| ProjectSummary {
                name: p.name.0.clone(),
                project_type: p.project_type.to_string(),
            })
            .collect();

        let tasks: Vec<TaskCtx> = codebase
            .projects
            .iter()
            .flat_map(|p| {
                p.tasks
                    .iter()
                    .filter(|t| !matches!(t.status, TaskStatus::Done))
                    .map(|t| TaskCtx {
                        id: t.id.0.clone(),
                        title: t.title.clone(),
                        status: format!("{:?}", t.status).to_lowercase(),
                        description: t.description.clone(),
                    })
            })
            .collect();

        let skills: Vec<SkillCtx> = codebase
            .projects
            .iter()
            .flat_map(|p| p.agents.iter())
            .filter_map(|a| a.skills.as_ref())
            .flat_map(|agent_skills| agent_skills.iter())
            .map(|skill| SkillCtx {
                id: skill.clone(),
                description: skill.clone(),
            })
            .collect();

        let subagents: Vec<SubagentCtx> = codebase
            .projects
            .iter()
            .flat_map(|p| p.agents.iter())
            .map(|agent| SubagentCtx {
                id: agent.agent_id.clone(),
                entry_point: agent.entry_point.display().to_string(),
                skills: agent.skills.clone().unwrap_or_default(),
            })
            .collect();

        let codebase_name = codebase.name.0.clone();
        let codebase_path = codebase.path.display().to_string();
        let active_task_count = tasks.len();

        TemplateContext {
            identity: IdentityCtx {
                codebase_name: codebase_name.clone(),
                codebase_path: codebase_path.clone(),
            },
            stack: StackCtx {
                projects: projects.clone(),
            },
            commands: CommandsCtx {
                sync: format!("orchestra sync {}", codebase_name),
                sync_dry_run: format!("orchestra sync {} --dry-run", codebase_name),
            },
            architecture: ArchitectureCtx {
                summary: "Refer to the project README and inline documentation.".to_string(),
            },
            conventions: Vec::new(),
            skills,
            tasks,
            subagents,
            meta: MetaCtx {
                orchestra_version: env!("CARGO_PKG_VERSION").to_string(),
                last_synced: None,
            },
            codebase_name,
            codebase_path,
            projects,
            active_task_count,
        }
    }

    /// Convert to a [`tera::Context`] for rendering.
    pub fn to_tera_context(&self) -> Result<tera::Context, RenderError> {
        tera::Context::from_serialize(self).map_err(RenderError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestra_core::types::{
        AgentConfig, Codebase, CodebaseName, Project, ProjectName, ProjectType, Task, TaskId,
    };
    use std::path::PathBuf;

    fn make_codebase(name: &str) -> Codebase {
        let now = Utc::now();
        Codebase {
            name: CodebaseName::from(name),
            path: PathBuf::from("/code/test"),
            projects: vec![Project {
                name: ProjectName::from("api"),
                project_type: ProjectType::Backend,
                tasks: vec![
                    Task {
                        id: TaskId::from("t-001"),
                        title: "Do thing".to_string(),
                        status: TaskStatus::Pending,
                        description: None,
                        created_at: now,
                        updated_at: now,
                    },
                    Task {
                        id: TaskId::from("t-002"),
                        title: "Done thing".to_string(),
                        status: TaskStatus::Done,
                        description: None,
                        created_at: now,
                        updated_at: now,
                    },
                ],
                agents: vec![AgentConfig {
                    agent_id: "coder".to_string(),
                    entry_point: PathBuf::from("AGENT/coder.md"),
                    skills: Some(vec!["rust".to_string()]),
                }],
            }],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn context_fields_populated() {
        let cb = make_codebase("myapp");
        let ctx = TemplateContext::from_codebase(&cb);
        assert_eq!(ctx.codebase_name, "myapp");
        assert_eq!(ctx.identity.codebase_name, "myapp");
        assert_eq!(ctx.projects.len(), 1);
        assert_eq!(ctx.stack.projects[0].project_type, "backend");
        assert_eq!(ctx.active_task_count, 1);
        assert_eq!(ctx.tasks.len(), 1, "done tasks must be filtered out");
        assert_eq!(ctx.skills.len(), 1);
        assert_eq!(ctx.subagents.len(), 1);
        assert!(ctx.meta.last_synced.is_none());
    }

    #[test]
    fn to_tera_context_succeeds() {
        let cb = make_codebase("tera_test");
        let ctx = TemplateContext::from_codebase(&cb);
        let tera_ctx = ctx.to_tera_context().expect("context conversion");
        let _ = tera_ctx;
    }
}
