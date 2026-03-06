//! Tera rendering engine — [`AgentKind`] enum and [`Renderer`].
//!
//! # Path mapping (official docs)
//!
//! | Agent       | Output path(s) under `./orchestra/controls/`                 |
//! |-------------|--------------------------------------------------------------|
//! | Claude      | `CLAUDE.md`, `.claude/rules/orchestra.md`, `.claude/agents/...` |
//! | Cursor      | `.cursor/rules/orchestra.mdc`, `.cursor/skills/orchestra-sync/skill.md` |
//! | Windsurf    | `.windsurf/rules/orchestra.md`, `.windsurf/skills/orchestra-sync/skill.md` |
//! | Copilot     | `.github/copilot-instructions.md`, `.github/instructions/orchestra.instructions.md` |
//! | Codex       | `AGENTS.md`, `.codex/skills/orchestra-sync/skill.md`        |
//! | Gemini      | `GEMINI.md`, `.gemini/settings.json`, `.gemini/styleguide.md`, `.gemini/skills/orchestra-sync/skill.md`|
//! | Cline       | `.clinerules/orchestra.md`, `.agents/skills/orchestra-sync/skill.md` |
//! | Antigravity | `.agent/rules/orchestra.md`, `.agent/skills/orchestra-sync/skill.md` |

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tera::Tera;

use orchestra_core::types::Codebase;

use crate::context::TemplateContext;
use crate::error::RenderError;

// ---------------------------------------------------------------------------
// Embedded templates — baked into the binary at compile time via include_str!
// ---------------------------------------------------------------------------

const TPLS: &[(&str, &str)] = &[
    ("shared/_header.tera", include_str!("templates/_partials/header.tera")),
    ("shared/_tasks.tera", include_str!("templates/_partials/tasks.tera")),
    ("shared/_stack.tera", include_str!("templates/_partials/stack.tera")),
    (
        "shared/_conventions_inline.tera",
        include_str!("templates/_partials/conventions_inline.tera"),
    ),
    (
        "shared/_conventions_section.tera",
        include_str!("templates/_partials/conventions_section.tera"),
    ),
    ("shared/_skills.tera", include_str!("templates/_partials/skills.tera")),
    (
        "shared/_orchestra_workflow.tera",
        include_str!("templates/_partials/orchestra_workflow.tera"),
    ),
    (
        "shared/_subagent_delegation.tera",
        include_str!("templates/_partials/subagent_delegation.tera"),
    ),
    (
        "shared/_worktree_instructions.tera",
        include_str!("templates/_partials/worktree_instructions.tera"),
    ),
    ("claude/claude.md.tera", include_str!("templates/claude.md.tera")),
    (
        "claude/rules.md.tera",
        include_str!("templates/claude_rules.md.tera"),
    ),
    (
        "claude/subagent-worker.md.tera",
        include_str!("templates/claude_subagent_worker.md.tera"),
    ),
    (
        "claude/subagent-reviewer.md.tera",
        include_str!("templates/claude_subagent_reviewer.md.tera"),
    ),
    ("cursor/cursorrules.tera", include_str!("templates/cursor.mdc.tera")),
    (
        "cursor/skill-orchestra-sync.md.tera",
        include_str!("templates/cursor_skill_orchestra_sync.md.tera"),
    ),
    ("windsurf/orchestra.md.tera", include_str!("templates/windsurf.md.tera")),
    (
        "windsurf/skill-orchestra-sync.md.tera",
        include_str!("templates/windsurf_skill_orchestra_sync.md.tera"),
    ),
    (
        "copilot/copilot-instructions.md.tera",
        include_str!("templates/copilot.md.tera"),
    ),
    (
        "copilot/orchestra.instructions.md.tera",
        include_str!("templates/copilot_path.instructions.md.tera"),
    ),
    ("codex/agents.md.tera", include_str!("templates/codex.md.tera")),
    (
        "codex/skill-orchestra-sync.md.tera",
        include_str!("templates/codex_skill_orchestra_sync.md.tera"),
    ),
    (
        "gemini/gemini.md.tera",
        include_str!("templates/gemini_instructions.md.tera"),
    ),
    (
        "gemini/settings.json.tera",
        include_str!("templates/gemini_settings.json.tera"),
    ),
    (
        "gemini/styleguide.md.tera",
        include_str!("templates/gemini_styleguide.md.tera"),
    ),
    (
        "gemini/skill-orchestra-sync.md.tera",
        include_str!("templates/gemini_skill_orchestra_sync.md.tera"),
    ),
    ("cline/orchestra.md.tera", include_str!("templates/cline.md.tera")),
    (
        "cline/skill-orchestra-sync.md.tera",
        include_str!("templates/cline_skill_orchestra_sync.md.tera"),
    ),
    (
        "antigravity/orchestra.md.tera",
        include_str!("templates/antigravity.md.tera"),
    ),
    (
        "antigravity/skill-orchestra-sync.md.tera",
        include_str!("templates/antigravity_skill_orchestra_sync.md.tera"),
    ),
    (
        "pilot/pilot.md.tera",
        include_str!("templates/pilot.md.tera"),
    ),
    (
        "guide/guide.md.tera",
        include_str!("templates/guide.md.tera"),
    ),
];

// ---------------------------------------------------------------------------
// Template loading helpers
// ---------------------------------------------------------------------------

fn io_err(path: impl Into<PathBuf>, source: std::io::Error) -> RenderError {
    RenderError::Io { path: path.into(), source }
}

fn normalize_template_name(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
}

fn collect_template_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), RenderError> {
    let entries = std::fs::read_dir(dir).map_err(|e| io_err(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_err(dir, e))?;
        let path = entry.path();
        let meta = entry.metadata().map_err(|e| io_err(&path, e))?;
        if meta.is_dir() {
            collect_template_files(&path, out)?;
        } else if meta.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn load_user_templates(dir: &Path) -> Result<Vec<(String, String)>, RenderError> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut files = Vec::new();
    collect_template_files(dir, &mut files)?;
    let mut templates = Vec::new();
    for path in files {
        if path.extension().and_then(|s| s.to_str()) != Some("tera") {
            continue;
        }
        let rel = path
            .strip_prefix(dir)
            .unwrap_or(path.as_path());
        let name = normalize_template_name(rel);
        let contents = std::fs::read_to_string(&path).map_err(|e| io_err(&path, e))?;
        templates.push((name, contents));
    }
    Ok(templates)
}

fn build_tera(user_template_dir: Option<&Path>) -> Result<Tera, RenderError> {
    let mut templates: HashMap<String, String> = HashMap::new();
    for (name, content) in TPLS {
        templates.insert(
            normalize_template_name(Path::new(name)),
            (*content).to_string(),
        );
    }
    if let Some(dir) = user_template_dir {
        for (name, content) in load_user_templates(dir)? {
            templates.insert(name, content);
        }
    }

    let mut tera = Tera::default();
    let items: Vec<(String, String)> = templates.into_iter().collect();
    tera.add_raw_templates(items)?;
    Ok(tera)
}

// ---------------------------------------------------------------------------
// AgentKind
// ---------------------------------------------------------------------------

/// All supported AI coding agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentKind {
    Claude,
    Cursor,
    Windsurf,
    Copilot,
    Codex,
    Gemini,
    Cline,
    Antigravity,
}

pub const PROJECT_ORCHESTRA_DIR: &str = "orchestra";
pub const LEGACY_PROJECT_ORCHESTRA_DIR: &str = ".orchestra";
pub const CONTROL_DIR_NAME: &str = "controls";
pub const BACKUP_DIR_NAME: &str = "backup";
pub const PILOT_FILE_NAME: &str = "pilot.md";
pub const GUIDE_FILE_NAME: &str = ".guide.md";

pub fn orchestra_dir(codebase_root: &Path) -> PathBuf {
    codebase_root.join(PROJECT_ORCHESTRA_DIR)
}

pub fn control_dir(codebase_root: &Path) -> PathBuf {
    orchestra_dir(codebase_root).join(CONTROL_DIR_NAME)
}

pub fn backup_dir(codebase_root: &Path) -> PathBuf {
    orchestra_dir(codebase_root).join(BACKUP_DIR_NAME)
}

pub fn pilot_path(codebase_root: &Path) -> PathBuf {
    orchestra_dir(codebase_root).join(PILOT_FILE_NAME)
}

pub fn guide_path(codebase_root: &Path) -> PathBuf {
    orchestra_dir(codebase_root).join(GUIDE_FILE_NAME)
}

pub fn legacy_orchestra_dirs(codebase_root: &Path) -> Vec<PathBuf> {
    vec![codebase_root.join(LEGACY_PROJECT_ORCHESTRA_DIR)]
}

impl AgentKind {
    /// All agent variants in a stable order.
    pub fn all() -> &'static [AgentKind] {
        &[
            AgentKind::Claude,
            AgentKind::Cursor,
            AgentKind::Windsurf,
            AgentKind::Copilot,
            AgentKind::Codex,
            AgentKind::Gemini,
            AgentKind::Cline,
            AgentKind::Antigravity,
        ]
    }

    /// Template name(s) to render for this agent.
    pub fn template_names(&self) -> &'static [&'static str] {
        match self {
            AgentKind::Claude      => &[
                "claude/claude.md.tera",
                "claude/rules.md.tera",
                "claude/subagent-worker.md.tera",
                "claude/subagent-reviewer.md.tera",
            ],
            AgentKind::Cursor      => &[
                "cursor/cursorrules.tera",
                "cursor/skill-orchestra-sync.md.tera",
            ],
            AgentKind::Windsurf    => &[
                "windsurf/orchestra.md.tera",
                "windsurf/skill-orchestra-sync.md.tera",
            ],
            AgentKind::Copilot     => &[
                "copilot/copilot-instructions.md.tera",
                "copilot/orchestra.instructions.md.tera",
            ],
            AgentKind::Codex       => &[
                "codex/agents.md.tera",
                "codex/skill-orchestra-sync.md.tera",
            ],
            AgentKind::Gemini      => &[
                "gemini/gemini.md.tera",
                "gemini/settings.json.tera",
                "gemini/styleguide.md.tera",
                "gemini/skill-orchestra-sync.md.tera",
            ],
            AgentKind::Cline       => &[
                "cline/orchestra.md.tera",
                "cline/skill-orchestra-sync.md.tera",
            ],
            AgentKind::Antigravity => &[
                "antigravity/orchestra.md.tera",
                "antigravity/skill-orchestra-sync.md.tera",
            ],
        }
    }

    /// Managed output paths for this agent, relative to the codebase root.
    /// Returns one `PathBuf` per template (same order as `template_names`).
    pub fn output_paths(&self, codebase_root: &Path) -> Vec<PathBuf> {
        let root = control_dir(codebase_root);
        match self {
            AgentKind::Claude => vec![
                root.join("CLAUDE.md"),
                root.join(".claude").join("rules").join("orchestra.md"),
                root
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-worker.md"),
                root
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-reviewer.md"),
            ],
            AgentKind::Cursor => vec![
                root.join(".cursor").join("rules").join("orchestra.mdc"),
                root
                    .join(".cursor")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Windsurf => vec![
                root.join(".windsurf").join("rules").join("orchestra.md"),
                root
                    .join(".windsurf")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Copilot => vec![
                root.join(".github").join("copilot-instructions.md"),
                root
                    .join(".github")
                    .join("instructions")
                    .join("orchestra.instructions.md"),
            ],
            AgentKind::Codex => vec![
                root.join("AGENTS.md"),
                root
                    .join(".codex")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Gemini => vec![
                root.join("GEMINI.md"),
                root.join(".gemini").join("settings.json"),
                root.join(".gemini").join("styleguide.md"),
                root
                    .join(".gemini")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Cline => vec![
                root.join(".clinerules").join("orchestra.md"),
                root
                    .join(".agents")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Antigravity => vec![
                root.join(".agent").join("rules").join("orchestra.md"),
                root
                    .join(".agent")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
        }
    }

    /// Legacy output paths used before Orchestra moved managed files under
    /// `./orchestra/controls/`. These remain for cleanup and migration.
    pub fn legacy_output_paths(&self, codebase_root: &Path) -> Vec<PathBuf> {
        let root = codebase_root;
        let hidden_legacy_controls = codebase_root
            .join(LEGACY_PROJECT_ORCHESTRA_DIR)
            .join("controls");
        let visible_legacy_controls = codebase_root
            .join(PROJECT_ORCHESTRA_DIR)
            .join("control");

        let mut paths = match self {
            AgentKind::Claude => vec![
                root.join("CLAUDE.md"),
                hidden_legacy_controls.join("CLAUDE.md"),
                visible_legacy_controls.join("CLAUDE.md"),
                root.join(".claude").join("rules").join("orchestra.md"),
                hidden_legacy_controls.join(".claude").join("rules").join("orchestra.md"),
                visible_legacy_controls.join(".claude").join("rules").join("orchestra.md"),
                root
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-worker.md"),
                hidden_legacy_controls
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-worker.md"),
                visible_legacy_controls
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-worker.md"),
                root
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-reviewer.md"),
                hidden_legacy_controls
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-reviewer.md"),
                visible_legacy_controls
                    .join(".claude")
                    .join("agents")
                    .join("orchestra-reviewer.md"),
            ],
            AgentKind::Cursor => vec![
                root.join(".cursor").join("rules").join("orchestra.mdc"),
                hidden_legacy_controls.join(".cursor").join("rules").join("orchestra.mdc"),
                visible_legacy_controls.join(".cursor").join("rules").join("orchestra.mdc"),
                root
                    .join(".cursor")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                hidden_legacy_controls
                    .join(".cursor")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                visible_legacy_controls
                    .join(".cursor")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Windsurf => vec![
                root.join(".windsurf").join("rules").join("orchestra.md"),
                hidden_legacy_controls.join(".windsurf").join("rules").join("orchestra.md"),
                visible_legacy_controls.join(".windsurf").join("rules").join("orchestra.md"),
                root
                    .join(".windsurf")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                hidden_legacy_controls
                    .join(".windsurf")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                visible_legacy_controls
                    .join(".windsurf")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Copilot => vec![
                root.join(".github").join("copilot-instructions.md"),
                hidden_legacy_controls.join(".github").join("copilot-instructions.md"),
                visible_legacy_controls.join(".github").join("copilot-instructions.md"),
                root
                    .join(".github")
                    .join("instructions")
                    .join("orchestra.instructions.md"),
                hidden_legacy_controls
                    .join(".github")
                    .join("instructions")
                    .join("orchestra.instructions.md"),
                visible_legacy_controls
                    .join(".github")
                    .join("instructions")
                    .join("orchestra.instructions.md"),
            ],
            AgentKind::Codex => vec![
                root.join("AGENTS.md"),
                hidden_legacy_controls.join("AGENTS.md"),
                visible_legacy_controls.join("AGENTS.md"),
                root
                    .join(".codex")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                hidden_legacy_controls
                    .join(".codex")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                visible_legacy_controls
                    .join(".codex")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Gemini => vec![
                root.join("GEMINI.md"),
                hidden_legacy_controls.join("GEMINI.md"),
                visible_legacy_controls.join("GEMINI.md"),
                root.join(".gemini").join("settings.json"),
                hidden_legacy_controls.join(".gemini").join("settings.json"),
                visible_legacy_controls.join(".gemini").join("settings.json"),
                root.join(".gemini").join("styleguide.md"),
                hidden_legacy_controls.join(".gemini").join("styleguide.md"),
                visible_legacy_controls.join(".gemini").join("styleguide.md"),
                root
                    .join(".gemini")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                hidden_legacy_controls
                    .join(".gemini")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                visible_legacy_controls
                    .join(".gemini")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Cline => vec![
                root.join(".clinerules").join("orchestra.md"),
                hidden_legacy_controls.join(".clinerules").join("orchestra.md"),
                visible_legacy_controls.join(".clinerules").join("orchestra.md"),
                root
                    .join(".agents")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                hidden_legacy_controls
                    .join(".agents")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                visible_legacy_controls
                    .join(".agents")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
            AgentKind::Antigravity => vec![
                root.join(".agent").join("rules").join("orchestra.md"),
                hidden_legacy_controls.join(".agent").join("rules").join("orchestra.md"),
                visible_legacy_controls.join(".agent").join("rules").join("orchestra.md"),
                root
                    .join(".agent")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                hidden_legacy_controls
                    .join(".agent")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
                visible_legacy_controls
                    .join(".agent")
                    .join("skills")
                    .join("orchestra-sync")
                    .join("skill.md"),
            ],
        };
        paths.sort();
        paths.dedup();
        paths
    }
}

// ---------------------------------------------------------------------------
// TemplateEngine
// ---------------------------------------------------------------------------

/// Tera-based engine for rendering templates with optional user overrides.
///
/// `user_template_dir` may contain `.tera` files that override embedded defaults.
/// Template names are normalised to lowercase and relative paths.
pub struct TemplateEngine {
    tera: Tera,
}

impl TemplateEngine {
    /// Construct a new [`TemplateEngine`], loading embedded templates plus any
    /// overrides found in `user_template_dir`.
    pub fn new(user_template_dir: Option<&Path>) -> Result<Self, RenderError> {
        let tera = build_tera(user_template_dir)?;
        Ok(TemplateEngine { tera })
    }

    /// Render all output files for a given `agent` using the supplied context.
    ///
    /// Returns `Vec<(output_path, rendered_content)>` — one entry per output file.
    pub fn render(
        &self,
        ctx: &TemplateContext,
        agent: AgentKind,
    ) -> Result<Vec<(PathBuf, String)>, RenderError> {
        let tera_ctx = ctx.to_tera_context()?;
        let codebase_root = Path::new(&ctx.codebase_path);
        let names = agent.template_names();
        let paths = agent.output_paths(codebase_root);

        debug_assert_eq!(
            names.len(),
            paths.len(),
            "template_names() and output_paths() must return equal-length slices for {:?}",
            agent
        );

        let mut results = Vec::with_capacity(names.len());
        for (name, path) in names.iter().zip(paths.into_iter()) {
            let content = self.tera.render(name, &tera_ctx)?;
            results.push((path, content));
        }
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Tera-based renderer for all agent kinds.
///
/// Uses embedded templates only. Create once with [`Renderer::new`] and reuse.
pub struct Renderer {
    engine: TemplateEngine,
}

impl Renderer {
    /// Construct a new [`Renderer`] with embedded templates.
    pub fn new() -> Result<Self, RenderError> {
        Ok(Renderer { engine: TemplateEngine::new(None)? })
    }

    /// Render all output files for a given `agent` using data from `codebase`.
    ///
    /// Returns `Vec<(output_path, rendered_content)>` — one entry per output file.
    pub fn render(
        &self,
        codebase: &Codebase,
        agent: AgentKind,
    ) -> Result<Vec<(PathBuf, String)>, RenderError> {
        let ctx = TemplateContext::from_codebase(codebase);
        self.render_with_context(&ctx, agent)
    }

    /// Render output files using a caller-provided [`TemplateContext`].
    pub fn render_with_context(
        &self,
        ctx: &TemplateContext,
        agent: AgentKind,
    ) -> Result<Vec<(PathBuf, String)>, RenderError> {
        self.engine.render(ctx, agent)
    }

    /// Render Orchestra pilot entrypoint file.
    pub fn render_pilot(&self, ctx: &TemplateContext) -> Result<(PathBuf, String), RenderError> {
        let tera_ctx = ctx.to_tera_context()?;
        let content = self.engine.tera.render("pilot/pilot.md.tera", &tera_ctx)?;
        let path = pilot_path(Path::new(&ctx.codebase_path));
        Ok((path, content))
    }

    /// Render Orchestra hidden context guide.
    pub fn render_guide(&self, ctx: &TemplateContext) -> Result<(PathBuf, String), RenderError> {
        let tera_ctx = ctx.to_tera_context()?;
        let content = self.engine.tera.render("guide/guide.md.tera", &tera_ctx)?;
        let path = guide_path(Path::new(&ctx.codebase_path));
        Ok((path, content))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use orchestra_core::types::{Codebase, CodebaseName, Project, ProjectName, ProjectType};
    use std::path::PathBuf;

    fn make_codebase(name: &str) -> Codebase {
        let now = Utc::now();
        Codebase {
            name: CodebaseName::from(name),
            path: PathBuf::from("/code").join(name),
            projects: vec![Project {
                name: ProjectName::from("api"),
                project_type: ProjectType::Backend,
                tasks: vec![],
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
    fn renderer_new_succeeds() {
        Renderer::new().expect("Renderer::new should succeed with embedded templates");
    }

    #[test]
    fn all_agents_render_without_error() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("testapp");
        for agent in AgentKind::all() {
            let results = renderer.render(&cb, *agent)
                .unwrap_or_else(|e| panic!("render failed for {:?}: {e}", agent));
            assert!(
                !results.is_empty(),
                "render() returned empty for {:?}",
                agent
            );
            for (_, content) in &results {
                assert!(
                    content.contains("testapp"),
                    "rendered content for {:?} should contain codebase name",
                    agent
                );
            }
        }
    }

    #[test]
    fn gemini_produces_four_outputs() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("multifile");
        let results = renderer.render(&cb, AgentKind::Gemini).unwrap();
        assert_eq!(results.len(), 4, "Gemini should produce exactly 4 output files");
    }

    #[test]
    fn claude_produces_context_rules_and_subagents() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("claudeapp");
        let results = renderer.render(&cb, AgentKind::Claude).unwrap();
        assert_eq!(results.len(), 4, "Claude should produce 4 files");
    }

    #[test]
    fn copilot_produces_repo_and_path_instructions() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("copilotapp");
        let results = renderer.render(&cb, AgentKind::Copilot).unwrap();
        assert_eq!(results.len(), 2, "Copilot should produce 2 files");
    }

    #[test]
    fn cline_produces_rules_and_skill_file() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("clineapp");
        let results = renderer.render(&cb, AgentKind::Cline).unwrap();
        assert_eq!(results.len(), 2, "Cline should produce 2 files");
    }

    #[test]
    fn cursor_windsurf_codex_gemini_antigravity_produce_skill_files() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("skillsapp");

        let cursor = renderer.render(&cb, AgentKind::Cursor).unwrap();
        assert_eq!(cursor.len(), 2, "Cursor should produce 2 files");

        let windsurf = renderer.render(&cb, AgentKind::Windsurf).unwrap();
        assert_eq!(windsurf.len(), 2, "Windsurf should produce 2 files");

        let codex = renderer.render(&cb, AgentKind::Codex).unwrap();
        assert_eq!(codex.len(), 2, "Codex should produce 2 files");

        let gemini = renderer.render(&cb, AgentKind::Gemini).unwrap();
        assert_eq!(gemini.len(), 4, "Gemini should produce 4 files");

        let antigravity = renderer.render(&cb, AgentKind::Antigravity).unwrap();
        assert_eq!(antigravity.len(), 2, "Antigravity should produce 2 files");
    }

    #[test]
    fn cursor_template_contains_frontmatter() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("cursorapp");
        let results = renderer.render(&cb, AgentKind::Cursor).unwrap();
        let content = &results[0].1;
        assert!(content.contains("alwaysApply: true"), "Cursor MDC must have alwaysApply frontmatter");
    }

    #[test]
    fn antigravity_template_contains_trigger() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("agapp");
        let results = renderer.render(&cb, AgentKind::Antigravity).unwrap();
        let content = &results[0].1;
        assert!(content.contains("trigger: always_on"), "Antigravity must have trigger: always_on in frontmatter");
    }

    #[test]
    fn output_paths_count_matches_template_count() {
        let root = PathBuf::from("/code/test");
        for agent in AgentKind::all() {
            assert_eq!(
                agent.template_names().len(),
                agent.output_paths(&root).len(),
                "path/template count mismatch for {:?}",
                agent
            );
        }
    }

    #[test]
    fn claude_output_path_is_correct() {
        let root = PathBuf::from("/code/myapp");
        let paths = AgentKind::Claude.output_paths(&root);
        assert_eq!(paths[0], PathBuf::from("/code/myapp/orchestra/controls/CLAUDE.md"));
    }

    #[test]
    fn copilot_output_path_is_correct() {
        let root = PathBuf::from("/code/myapp");
        let paths = AgentKind::Copilot.output_paths(&root);
        assert_eq!(
            paths[0],
            PathBuf::from("/code/myapp/orchestra/controls/.github/copilot-instructions.md")
        );
    }

    #[test]
    fn legacy_output_paths_remain_available_for_cleanup() {
        let root = PathBuf::from("/code/myapp");
        let paths = AgentKind::Claude.legacy_output_paths(&root);
        assert!(paths.contains(&PathBuf::from("/code/myapp/CLAUDE.md")));
        assert!(paths.contains(&PathBuf::from(
            "/code/myapp/.orchestra/controls/CLAUDE.md"
        )));
        assert!(paths.contains(&PathBuf::from(
            "/code/myapp/orchestra/control/CLAUDE.md"
        )));
    }

    #[test]
    fn gemini_settings_json_is_valid_json() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("json_test");
        let results = renderer.render(&cb, AgentKind::Gemini).unwrap();
        // settings.json is the second Gemini output (index 1).
        let settings_content = &results[1].1;
        serde_json::from_str::<serde_json::Value>(settings_content)
            .unwrap_or_else(|e| {
                panic!("Gemini settings.json rendered invalid JSON.\nError: {e}\nContent:\n{settings_content}")
            });
    }

    #[test]
    fn no_crlf_in_any_rendered_output() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("lineend_test");
        for agent in AgentKind::all() {
            let results = renderer.render(&cb, *agent).unwrap();
            for (path, content) in &results {
                assert!(
                    !content.contains('\r'),
                    "Rendered output for {:?} ({}) contains CR char — line endings not normalised",
                    agent,
                    path.display()
                );
            }
        }
    }
}
