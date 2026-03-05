//! Applier — applies parsed [`WritebackCommand`]s to a [`Codebase`] struct.

use chrono::Utc;
use orchestra_core::types::{Codebase, Skill, TaskStatus};

use crate::writeback::types::{ApplyOutcome, ApplyResult, WritebackCommand};

pub fn apply(codebase: &mut Codebase, commands: &[WritebackCommand]) -> Vec<ApplyResult> {
    let mut results = Vec::with_capacity(commands.len());

    for cmd in commands {
        results.push(apply_one(codebase, cmd));
    }

    let any_mutating_apply = results.iter().any(|result| {
        matches!(
            result.outcome,
            ApplyOutcome::Applied | ApplyOutcome::Skipped { .. }
        )
    });

    if any_mutating_apply {
        codebase.updated_at = Utc::now();
    }

    results
}

fn apply_one(codebase: &mut Codebase, cmd: &WritebackCommand) -> ApplyResult {
    match cmd {
        WritebackCommand::CodebaseHint { codebase } => ApplyResult {
            command: format!("codebase_hint: {codebase}"),
            outcome: ApplyOutcome::Applied,
        },
        WritebackCommand::TaskCompleted { task_id } => match update_task_status(codebase, task_id, TaskStatus::Done)
        {
            Ok(()) => ApplyResult {
                command: format!("task_completed: {task_id}"),
                outcome: ApplyOutcome::Applied,
            },
            Err(message) => ApplyResult {
                command: format!("task_completed: {task_id}"),
                outcome: ApplyOutcome::Error { message },
            },
        },
        WritebackCommand::TaskStarted { task_id } => match update_task_status(codebase, task_id, TaskStatus::InProgress)
        {
            Ok(()) => ApplyResult {
                command: format!("task_started: {task_id}"),
                outcome: ApplyOutcome::Applied,
            },
            Err(message) => ApplyResult {
                command: format!("task_started: {task_id}"),
                outcome: ApplyOutcome::Error { message },
            },
        },
        WritebackCommand::TaskBlocked { task_id, reason } => {
            let mut found = false;
            for project in &mut codebase.projects {
                for task in &mut project.tasks {
                    if task.id.0 == *task_id {
                        task.status = TaskStatus::Blocked;
                        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
                        task.notes.push(format!("[{timestamp}] Blocked: {reason}"));
                        task.updated_at = Utc::now();
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            if found {
                ApplyResult {
                    command: format!("task_blocked: {task_id} | {reason}"),
                    outcome: ApplyOutcome::Applied,
                }
            } else {
                ApplyResult {
                    command: format!("task_blocked: {task_id} | {reason}"),
                    outcome: ApplyOutcome::Error {
                        message: format!("task '{task_id}' not found in any project"),
                    },
                }
            }
        }
        WritebackCommand::SubtaskDone {
            task_id,
            subtask_title,
        } => {
            for project in &mut codebase.projects {
                for task in &mut project.tasks {
                    if task.id.0 == *task_id {
                        if let Some(subtask) = task
                            .subtasks
                            .iter_mut()
                            .find(|subtask| subtask.title == *subtask_title)
                        {
                            subtask.done = true;
                            task.updated_at = Utc::now();
                            return ApplyResult {
                                command: format!("subtask_done: {task_id}/{subtask_title}"),
                                outcome: ApplyOutcome::Applied,
                            };
                        }
                        return ApplyResult {
                            command: format!("subtask_done: {task_id}/{subtask_title}"),
                            outcome: ApplyOutcome::Error {
                                message: format!(
                                    "subtask '{subtask_title}' not found under task '{task_id}'"
                                ),
                            },
                        };
                    }
                }
            }

            ApplyResult {
                command: format!("subtask_done: {task_id}/{subtask_title}"),
                outcome: ApplyOutcome::Error {
                    message: format!("task '{task_id}' not found in any project"),
                },
            }
        }
        WritebackCommand::ConventionAdded { text } => {
            if codebase.conventions.iter().any(|convention| convention == text) {
                return ApplyResult {
                    command: format!("convention_added: {text}"),
                    outcome: ApplyOutcome::Skipped {
                        reason: "convention already present".to_owned(),
                    },
                };
            }
            codebase.conventions.push(text.clone());
            ApplyResult {
                command: format!("convention_added: {text}"),
                outcome: ApplyOutcome::Applied,
            }
        }
        WritebackCommand::SkillDiscovered { id, description } => {
            if codebase.skills.iter().any(|skill| skill.id == *id) {
                return ApplyResult {
                    command: format!("skill_discovered: {id} | {description}"),
                    outcome: ApplyOutcome::Skipped {
                        reason: format!("skill '{id}' already present"),
                    },
                };
            }
            codebase.skills.push(Skill {
                id: id.clone(),
                description: description.clone(),
            });
            ApplyResult {
                command: format!("skill_discovered: {id} | {description}"),
                outcome: ApplyOutcome::Applied,
            }
        }
        WritebackCommand::Note { text } => {
            let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
            codebase.notes.push(format!("[{timestamp}] {text}"));
            ApplyResult {
                command: format!("note: {text}"),
                outcome: ApplyOutcome::Applied,
            }
        }
        WritebackCommand::SubagentUsed { id } => ApplyResult {
            command: format!("subagent_used: {id}"),
            outcome: ApplyOutcome::Applied,
        },
        WritebackCommand::FileCreated { path } => {
            if codebase.tracked_files.iter().any(|tracked| tracked == path) {
                return ApplyResult {
                    command: format!("file_created: {}", path.display()),
                    outcome: ApplyOutcome::Skipped {
                        reason: "tracked file already present".to_owned(),
                    },
                };
            }
            codebase.tracked_files.push(path.clone());
            ApplyResult {
                command: format!("file_created: {}", path.display()),
                outcome: ApplyOutcome::Applied,
            }
        }
        WritebackCommand::FileDeleted { path } => {
            let original_len = codebase.tracked_files.len();
            codebase.tracked_files.retain(|tracked| tracked != path);
            if codebase.tracked_files.len() == original_len {
                return ApplyResult {
                    command: format!("file_deleted: {}", path.display()),
                    outcome: ApplyOutcome::Skipped {
                        reason: "tracked file not present".to_owned(),
                    },
                };
            }
            ApplyResult {
                command: format!("file_deleted: {}", path.display()),
                outcome: ApplyOutcome::Applied,
            }
        }
    }
}

fn update_task_status(
    codebase: &mut Codebase,
    task_id: &str,
    status: TaskStatus,
) -> Result<(), String> {
    for project in &mut codebase.projects {
        for task in &mut project.tasks {
            if task.id.0 == task_id {
                task.status = status;
                task.updated_at = Utc::now();
                return Ok(());
            }
        }
    }
    Err(format!("task '{task_id}' not found in any project"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use orchestra_core::types::{
        Codebase, CodebaseName, Project, ProjectName, ProjectType, Subtask, Task, TaskId,
        TaskStatus,
    };
    use std::path::PathBuf;

    fn make_task(id: &str) -> Task {
        let now = Utc::now();
        Task {
            id: TaskId::from(id),
            title: format!("Task {id}"),
            status: TaskStatus::Pending,
            description: None,
            subtasks: vec![Subtask {
                title: "Write tests".to_owned(),
                done: false,
            }],
            notes: vec![],
            created_at: now,
            updated_at: now,
        }
    }

    fn make_codebase() -> Codebase {
        let now = Utc::now();
        Codebase {
            name: CodebaseName::from("test_cb"),
            path: PathBuf::from("/tmp/test_cb"),
            projects: vec![Project {
                name: ProjectName::from("default"),
                project_type: ProjectType::Backend,
                tasks: vec![make_task("T-1")],
                agents: vec![],
            }],
            conventions: vec![],
            skills: vec![],
            notes: vec![],
            tracked_files: vec![],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn applies_task_started_and_task_blocked() {
        let mut codebase = make_codebase();
        let results = apply(
            &mut codebase,
            &[
                WritebackCommand::TaskStarted {
                    task_id: "T-1".to_owned(),
                },
                WritebackCommand::TaskBlocked {
                    task_id: "T-1".to_owned(),
                    reason: "Waiting on dependency".to_owned(),
                },
            ],
        );

        assert!(matches!(results[0].outcome, ApplyOutcome::Applied));
        assert!(matches!(results[1].outcome, ApplyOutcome::Applied));
        let task = &codebase.projects[0].tasks[0];
        assert_eq!(task.status, TaskStatus::Blocked);
        assert!(task
            .notes
            .iter()
            .any(|note| note.contains("Waiting on dependency")));
    }

    #[test]
    fn applies_subtask_done() {
        let mut codebase = make_codebase();
        let results = apply(
            &mut codebase,
            &[WritebackCommand::SubtaskDone {
                task_id: "T-1".to_owned(),
                subtask_title: "Write tests".to_owned(),
            }],
        );

        assert!(matches!(results[0].outcome, ApplyOutcome::Applied));
        assert!(codebase.projects[0].tasks[0].subtasks[0].done);
    }

    #[test]
    fn records_subagent_used_without_registry_structural_changes() {
        let mut codebase = make_codebase();
        let before = codebase.clone();
        let results = apply(
            &mut codebase,
            &[WritebackCommand::SubagentUsed {
                id: "qa-reliability-reviewer".to_owned(),
            }],
        );
        assert!(matches!(results[0].outcome, ApplyOutcome::Applied));
        assert_eq!(before.tracked_files, codebase.tracked_files);
        assert_eq!(before.conventions, codebase.conventions);
        assert_eq!(before.skills, codebase.skills);
        assert_eq!(before.notes, codebase.notes);
    }

    #[test]
    fn file_created_and_deleted_lifecycle() {
        let mut codebase = make_codebase();
        let path = PathBuf::from("docs/new.md");

        let created = apply(
            &mut codebase,
            &[WritebackCommand::FileCreated {
                path: path.clone(),
            }],
        );
        assert!(matches!(created[0].outcome, ApplyOutcome::Applied));
        assert!(codebase.tracked_files.contains(&path));

        let deleted = apply(
            &mut codebase,
            &[WritebackCommand::FileDeleted {
                path: path.clone(),
            }],
        );
        assert!(matches!(deleted[0].outcome, ApplyOutcome::Applied));
        assert!(!codebase.tracked_files.contains(&path));
    }

    #[test]
    fn applies_remaining_commands() {
        let mut codebase = make_codebase();
        let results = apply(
            &mut codebase,
            &[
                WritebackCommand::TaskCompleted {
                    task_id: "T-1".to_owned(),
                },
                WritebackCommand::ConventionAdded {
                    text: "Always run tests".to_owned(),
                },
                WritebackCommand::SkillDiscovered {
                    id: "rust-async".to_owned(),
                    description: "Async orchestration".to_owned(),
                },
                WritebackCommand::Note {
                    text: "Captured context".to_owned(),
                },
            ],
        );
        assert_eq!(results.len(), 4);
        assert_eq!(codebase.projects[0].tasks[0].status, TaskStatus::Done);
        assert_eq!(codebase.conventions.len(), 1);
        assert_eq!(codebase.skills.len(), 1);
        assert_eq!(codebase.notes.len(), 1);
    }
}
