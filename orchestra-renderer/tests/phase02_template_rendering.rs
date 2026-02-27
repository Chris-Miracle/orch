use chrono::Utc;
use orchestra_core::types::{
    Codebase, CodebaseName, Project, ProjectName, ProjectType, Task, TaskId, TaskStatus,
};
use orchestra_renderer::{
    AgentKind, TemplateContext, TemplateEngine,
    context::{SkillCtx, TaskSummary},
};
use tempfile::TempDir;

fn make_codebase() -> Codebase {
    let now = Utc::now();
    Codebase {
        name: CodebaseName::from("copnow_api"),
        path: std::path::PathBuf::from("/code/copnow_api"),
        projects: vec![Project {
            name: ProjectName::from("api"),
            project_type: ProjectType::Backend,
            tasks: vec![
                Task {
                    id: TaskId::from("t-001"),
                    title: "Pending: auth flow".to_string(),
                    status: TaskStatus::Pending,
                    description: Some("Implement login + session handling".to_string()),
                    created_at: now,
                    updated_at: now,
                },
                Task {
                    id: TaskId::from("t-002"),
                    title: "In Progress: search indexing".to_string(),
                    status: TaskStatus::InProgress,
                    description: None,
                    created_at: now,
                    updated_at: now,
                },
                Task {
                    id: TaskId::from("t-003"),
                    title: "Blocked: refactor logging".to_string(),
                    status: TaskStatus::Blocked,
                    description: Some("Waiting on infra change".to_string()),
                    created_at: now,
                    updated_at: now,
                },
                Task {
                    id: TaskId::from("t-004"),
                    title: "Done: remove legacy code".to_string(),
                    status: TaskStatus::Done,
                    description: Some("Delete deprecated endpoints".to_string()),
                    created_at: now,
                    updated_at: now,
                },
            ],
            agents: vec![],
        }],
        created_at: now,
        updated_at: now,
    }
}

fn expected_conventions(agent: AgentKind) -> &'static [&'static str] {
    match agent {
        AgentKind::Claude => &[
            "Follow existing code style — match indentation, naming, and module structure.",
            "Never introduce new dependencies without confirming necessity.",
            "Keep changes minimal and focused; avoid unrelated refactors.",
            "Run the project's test suite before marking a task complete.",
            "Prefer small, reviewable commits over large sweeping changes.",
        ],
        AgentKind::Cursor => &[
            "Match existing code style exactly — do not reformat unrelated code.",
            "Do not add dependencies without explicit instruction.",
            "Prefer small, targeted edits over full-file rewrites.",
            "Run tests before marking any task done.",
            "Keep commits atomic and focused.",
        ],
        AgentKind::Windsurf => &[
            "Match existing style — indentation, naming, file structure.",
            "No new dependencies unless explicitly requested.",
            "Minimal, focused changes — avoid scope creep.",
            "All tests must pass before task completion.",
            "Prefer incremental commits; avoid large sweeps.",
        ],
        AgentKind::Copilot => &[
            "Follow the existing file structure and naming conventions already in the codebase.",
            "Match the language and framework idioms present in each directory.",
            "When creating new files, mirror the style of adjacent files.",
            "Generate complete, working implementations — avoid TODO stubs unless explicitly asked.",
            "Do not introduce dependencies not already in the project.",
            "Do not reformat code outside the lines you are changing.",
            "Do not generate overly verbose comments — match the existing comment density.",
            "Do not change unrelated files when asked to modify a specific feature.",
            "Prefer the patterns already established in the codebase.",
            "Keep functions focused and small.",
            "All public APIs must have documentation comments.",
            "Tests must be written for all new business logic.",
        ],
        AgentKind::Codex => &[
            "**Style:** Match existing conventions in each file. Do not reformat unrelated code.",
            "**Dependencies:** Do not add new libraries without explicit approval.",
            "**Scope:** Keep changes minimal and targeted; avoid unrelated modifications.",
            "**Tests:** All new code must have corresponding tests. Run the full test suite before marking a task done.",
            "**Commits:** Write clear commit messages describing *what* changed and *why*.",
            "**PRs:** Keep pull requests small and focused on a single concern.",
            "**Security:** Never commit secrets, credentials, or API keys.",
        ],
        AgentKind::Gemini => &[
            "Follow existing code style — indentation, naming, module layout.",
            "Do not add dependencies without explicit instruction.",
            "Keep changes minimal and focused.",
            "Run the project test suite before marking tasks done.",
            "Match the style of adjacent files in every edit.",
            "Preserve existing indentation — do not reformat beyond the changed lines.",
            "Keep line length consistent with the rest of the file.",
            "Use descriptive names — avoid single-letter variables outside short loop indices.",
            "Follow the naming convention of the language in use (snake_case, camelCase, PascalCase, etc.).",
            "All public functions and types must have documentation comments.",
            "Comments should explain *why*, not *what* — the code shows what.",
            "Every new business-logic function requires at least one unit test.",
            "Test edge cases: empty inputs, boundary values, and error paths.",
            "Never commit secrets, credentials, or API keys.",
            "Validate all inputs on the server side.",
            "Use parameterized queries — never string-interpolate user input into SQL.",
        ],
        AgentKind::Cline => &[
            "Match existing code style — indentation, naming, structure.",
            "Do not add dependencies without explicit instruction.",
            "Keep changes minimal and scoped to the task.",
            "Do not modify files unrelated to the current task.",
            "Run all tests before marking a task done.",
            "Write documentation for every new public function or type.",
            "Never commit secrets, credentials, or API keys.",
            "Prefer small, focused PRs over large sweeping changes.",
        ],
        AgentKind::Antigravity => &[
            "Match existing code style — do not reformat adjacent code.",
            "Do not add dependencies without explicit instruction.",
            "Keep edits minimal and targeted to the request.",
            "All new code requires tests. Run the full test suite before completion.",
            "Write clear commit messages explaining what changed and why.",
            "Never commit secrets, credentials, or API keys.",
            "When uncertain about approach, ask for clarification.",
        ],
    }
}

#[test]
fn template_rendering_correctness_all_agents() {
    let codebase = make_codebase();
    let mut ctx = TemplateContext::from_codebase(&codebase);
    let extra_conventions = vec![
        "No tabs — spaces only.".to_string(),
        "Prefer explicit error handling over unwraps.".to_string(),
    ];
    let extra_skills = vec![
        SkillCtx {
            id: "orchestra-template-rendering".to_string(),
            description: "orchestra-template-rendering".to_string(),
        },
        SkillCtx {
            id: "template-sync-engineer".to_string(),
            description: "template-sync-engineer".to_string(),
        },
    ];
    ctx.conventions = extra_conventions.clone();
    ctx.skills = extra_skills.clone();
    let engine = TemplateEngine::new(None).expect("engine");

    let active_titles = [
        "Pending: auth flow",
        "In Progress: search indexing",
        "Blocked: refactor logging",
    ];
    let done_title = "Done: remove legacy code";
    let done_id = "t-004";

    for agent in AgentKind::all() {
        let outputs = engine.render(&ctx, *agent)
            .unwrap_or_else(|e| panic!("render failed for {:?}: {e}", agent));
        let mut combined = String::new();
        for (_, content) in &outputs {
            combined.push_str(content);
            combined.push('\n');
        }

        assert!(
            combined.contains("copnow_api"),
            "codebase name missing for {:?}",
            agent
        );
        for title in active_titles {
            assert!(
                combined.contains(title),
                "active task '{title}' missing for {:?}",
                agent
            );
        }
        assert!(
            !combined.contains(done_title),
            "done task title leaked for {:?}",
            agent
        );
        assert!(
            !combined.contains(done_id),
            "done task id leaked for {:?}",
            agent
        );
        for conv in expected_conventions(*agent) {
            assert!(
                combined.contains(conv),
                "convention missing for {:?}: {conv}",
                agent
            );
        }
        for conv in &extra_conventions {
            assert!(
                combined.contains(conv),
                "custom convention missing for {:?}: {conv}",
                agent
            );
        }
        for skill in &extra_skills {
            assert!(
                combined.contains(&skill.description),
                "skill missing for {:?}: {}",
                agent,
                skill.description
            );
        }

        if *agent == AgentKind::Gemini {
            let settings = outputs.iter()
                .find(|(path, _)| path.ends_with("settings.json"))
                .expect("gemini settings.json output missing");
            serde_json::from_str::<serde_json::Value>(&settings.1)
                .unwrap_or_else(|e| {
                    panic!("Gemini settings.json invalid JSON: {e}\nContent:\n{}", settings.1)
                });
        }
    }
}

#[test]
fn user_template_override_wins() {
    let codebase = make_codebase();
    let ctx = TemplateContext::from_codebase(&codebase);
    let dir = TempDir::new().expect("tempdir");
    let custom = "# Custom CLAUDE template for {{ codebase_name }}\n";
    let custom_path = dir.path().join("claude").join("claude.md.tera");
    std::fs::create_dir_all(custom_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(custom_path, custom).expect("write custom template");

    let engine = TemplateEngine::new(Some(dir.path())).expect("engine");
    let outputs = engine.render(&ctx, AgentKind::Claude).expect("render");
    let content = &outputs[0].1;

    assert!(content.contains("Custom CLAUDE template"), "custom template not used");
    assert!(content.contains("copnow_api"), "custom template missing context");
    assert!(!content.contains("Project Overview"), "embedded template leaked through");
}

#[test]
fn meta_last_synced_is_stable_without_sync() {
    let codebase = make_codebase();
    let mut ctx = TemplateContext::from_codebase(&codebase);
    let last_synced = chrono::DateTime::parse_from_rfc3339("2026-02-27T15:30:00Z")
        .unwrap()
        .with_timezone(&Utc);
    ctx.meta.last_synced = Some(last_synced);

    let dir = TempDir::new().expect("tempdir");
    let template_path = dir.path().join("claude").join("claude.md.tera");
    std::fs::create_dir_all(template_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &template_path,
        "last={{ meta.last_synced }};version={{ meta.orchestra_version }}",
    )
    .expect("write template");

    let engine = TemplateEngine::new(Some(dir.path())).expect("engine");
    let first = engine.render(&ctx, AgentKind::Claude).expect("render #1");
    let second = engine.render(&ctx, AgentKind::Claude).expect("render #2");
    assert_eq!(first[0].1, second[0].1);

    let mut changed = ctx.clone();
    changed.meta.last_synced = Some(last_synced + chrono::Duration::seconds(1));
    let third = engine.render(&changed, AgentKind::Claude).expect("render #3");
    assert_ne!(first[0].1, third[0].1);
}

#[test]
fn rendering_handles_many_string_shapes() {
    let sample_sets: &[&[&str]] = &[
        &["", "simple", "CAPS", "snake_case", "kebab-case"],
        &["emoji-rocket-🚀", "quotes-'\"`", "braces-{}[]()", "slash-\\", "pipes-||"],
        &["arabic-مرحبا", "japanese-日本語", "accents-éèà", "math-<= >= !=", "long-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"],
    ];

    for conventions in sample_sets {
        for titles in sample_sets {
            for skills in sample_sets {
                let tasks: Vec<TaskSummary> = titles.iter().enumerate().map(|(idx, title)| {
                    TaskSummary {
                        id: format!("t-{idx}"),
                        title: (*title).to_string(),
                        status: "pending".to_string(),
                        description: None,
                    }
                }).collect();

                let mut ctx = TemplateContext::from_codebase(&make_codebase());
                ctx.tasks = tasks;
                ctx.conventions = conventions.iter().map(|s| (*s).to_string()).collect();
                ctx.skills = skills
                    .iter()
                    .map(|s| SkillCtx {
                        id: (*s).to_string(),
                        description: (*s).to_string(),
                    })
                    .collect();
                ctx.active_task_count = titles.len();

                let dir = TempDir::new().expect("tempdir");
                let tpl = r#"
{% for c in conventions %}CONV: {{ c }}
{% endfor %}{% for s in skills %}SKILL: {{ s.description }}
{% endfor %}{% for t in tasks %}TASK: {{ t.title }}
{% endfor %}"#;
                let template_path = dir.path().join("claude").join("claude.md.tera");
                std::fs::create_dir_all(template_path.parent().expect("parent")).expect("mkdir");
                std::fs::write(template_path, tpl).expect("write tpl");
                let engine = TemplateEngine::new(Some(dir.path())).expect("engine");
                let outputs = engine.render(&ctx, AgentKind::Claude).expect("render");
                let rendered = &outputs[0].1;
                assert!(std::str::from_utf8(rendered.as_bytes()).is_ok());
            }
        }
    }
}
