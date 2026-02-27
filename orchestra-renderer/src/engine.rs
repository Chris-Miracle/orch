//! Tera rendering engine — [`AgentKind`] enum and [`Renderer`].
//!
//! # Path mapping (official docs)
//!
//! | Agent       | Output path(s)                                               |
//! |-------------|--------------------------------------------------------------|
//! | Claude      | `CLAUDE.md`                                                  |
//! | Cursor      | `.cursor/rules/orchestra.mdc`                                |
//! | Windsurf    | `.windsurf/rules/orchestra.md`                               |
//! | Copilot     | `.github/copilot-instructions.md`                            |
//! | Codex       | `AGENTS.md`                                                  |
//! | Gemini      | `GEMINI.md`, `.gemini/settings.json`, `.gemini/styleguide.md`|
//! | Cline       | `.clinerules/orchestra.md`                                   |
//! | Antigravity | `.agent/rules/orchestra.md`                                  |

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
    ("claude/claude.md.tera", include_str!("templates/claude.md.tera")),
    ("cursor/cursorrules.tera", include_str!("templates/cursor.mdc.tera")),
    ("windsurf/orchestra.md.tera", include_str!("templates/windsurf.md.tera")),
    (
        "copilot/copilot-instructions.md.tera",
        include_str!("templates/copilot.md.tera"),
    ),
    ("codex/agents.md.tera", include_str!("templates/codex.md.tera")),
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
    ("cline/orchestra.md.tera", include_str!("templates/cline.md.tera")),
    (
        "antigravity/orchestra.md.tera",
        include_str!("templates/antigravity.md.tera"),
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
    /// Gemini produces three files; all others produce one.
    pub fn template_names(&self) -> &'static [&'static str] {
        match self {
            AgentKind::Claude      => &["claude/claude.md.tera"],
            AgentKind::Cursor      => &["cursor/cursorrules.tera"],
            AgentKind::Windsurf    => &["windsurf/orchestra.md.tera"],
            AgentKind::Copilot     => &["copilot/copilot-instructions.md.tera"],
            AgentKind::Codex       => &["codex/agents.md.tera"],
            AgentKind::Gemini      => &[
                "gemini/gemini.md.tera",
                "gemini/settings.json.tera",
                "gemini/styleguide.md.tera",
            ],
            AgentKind::Cline       => &["cline/orchestra.md.tera"],
            AgentKind::Antigravity => &["antigravity/orchestra.md.tera"],
        }
    }

    /// Official output paths for this agent, relative to the codebase root.
    /// Returns one `PathBuf` per template (same order as `template_names`).
    pub fn output_paths(&self, codebase_root: &Path) -> Vec<PathBuf> {
        let root = codebase_root;
        match self {
            AgentKind::Claude => vec![
                root.join("CLAUDE.md"),
            ],
            AgentKind::Cursor => vec![
                root.join(".cursor").join("rules").join("orchestra.mdc"),
            ],
            AgentKind::Windsurf => vec![
                root.join(".windsurf").join("rules").join("orchestra.md"),
            ],
            AgentKind::Copilot => vec![
                root.join(".github").join("copilot-instructions.md"),
            ],
            AgentKind::Codex => vec![
                root.join("AGENTS.md"),
            ],
            AgentKind::Gemini => vec![
                root.join("GEMINI.md"),
                root.join(".gemini").join("settings.json"),
                root.join(".gemini").join("styleguide.md"),
            ],
            AgentKind::Cline => vec![
                root.join(".clinerules").join("orchestra.md"),
            ],
            AgentKind::Antigravity => vec![
                root.join(".agent").join("rules").join("orchestra.md"),
            ],
        }
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
    fn gemini_produces_three_outputs() {
        let renderer = Renderer::new().unwrap();
        let cb = make_codebase("multifile");
        let results = renderer.render(&cb, AgentKind::Gemini).unwrap();
        assert_eq!(results.len(), 3, "Gemini should produce exactly 3 output files");
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
        assert_eq!(paths[0], PathBuf::from("/code/myapp/CLAUDE.md"));
    }

    #[test]
    fn copilot_output_path_is_correct() {
        let root = PathBuf::from("/code/myapp");
        let paths = AgentKind::Copilot.output_paths(&root);
        assert_eq!(paths[0], PathBuf::from("/code/myapp/.github/copilot-instructions.md"));
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
