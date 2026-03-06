//! `orchestra onboard` — interactive onboarding and bootstrap.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use orchestra_core::{
    registry,
    types::{ProjectName, ProjectType, Task, TaskId, TaskStatus},
};
use orchestra_detector::{detect_stack, scan_agent_files, AgentFileHit};
use orchestra_renderer::engine::{backup_dir, control_dir, guide_path, orchestra_dir, pilot_path};
use orchestra_sync::{
    backup_agent_files, pipeline, BackupItem, SyncScope,
};

const IMPORT_BLOCK_START: &str = "<!-- orchestra:import ";
const IMPORT_BLOCK_END: &str = "<!-- /orchestra:import -->";

// ---------------------------------------------------------------------------
// Migrate mode
// ---------------------------------------------------------------------------

/// How to handle existing agent files during onboarding.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MigrateMode {
    /// Generate a ready-to-paste prompt for the user's agent chat (recommended).
    #[default]
    Prompt,
    /// Orchestra mechanically preserves all user files and merges content into the registry.
    Mechanical,
}

impl std::str::FromStr for MigrateMode {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "prompt" => Ok(MigrateMode::Prompt),
            "mechanical" => Ok(MigrateMode::Mechanical),
            other => Err(format!(
                "unknown migrate mode '{other}'; expected: prompt, mechanical"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

/// Arguments for `orchestra onboard`.
#[derive(Args, Debug)]
pub struct OnboardArgs {
    /// Optional path to codebase root (defaults to current directory).
    pub path: Option<PathBuf>,

    /// Project group name. If omitted, interactive prompt is used.
    #[arg(long, short = 'p')]
    pub project: Option<String>,

    /// Accept detected/default project type without prompt.
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Re-run onboarding even if this path is already registered.
    #[arg(long)]
    pub force: bool,

    /// Migration mode for existing agent files: "prompt" (recommended) or "mechanical".
    ///
    /// - prompt:     Generates a one-shot setup prompt you paste into your agent chat.
    ///               The agent reads official docs, migrates your skills/subagents/rules,
    ///               and sets up pilot.md as the master orchestrator. (recommended)
    ///
    /// - mechanical: Orchestra preserves all your existing agent files in-place and merges
    ///               discovered conventions and notes into the registry automatically.
    #[arg(long)]
    pub migrate: Option<String>,

    /// Delete legacy agent files and folders after successful onboarding import.
    #[arg(long)]
    pub delete: bool,
}

impl OnboardArgs {
    pub fn run(self) -> Result<()> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let raw_path = self.path.unwrap_or_else(|| PathBuf::from("."));
        let codebase_path = raw_path
            .canonicalize()
            .with_context(|| format!("cannot resolve path '{}'", raw_path.display()))?;
        let codebases = registry::list_codebases_at(&home).context("failed to read registry")?;
        if let Some((project, cb)) = codebases.iter().find(|(_, cb)| cb.path == codebase_path) {
            if !self.force {
                println!(
                    "Already onboarded as '{}' under project '{}'.",
                    cb.name, project
                );
                println!("Use --force to re-run onboarding workflow.");
                return Ok(());
            }
        }

        let project_type = resolve_project_type(&codebase_path, self.yes)?;
        let project_name = resolve_project_name(self.project, &home, &codebase_path)?;
        let project_key = ProjectName::from(project_name.clone());

        let mut codebase = registry::init_at(
            codebase_path.clone(),
            project_key.clone(),
            Some(project_type),
            &home,
        )
        .with_context(|| {
            format!(
                "failed to init '{}' under project '{}'",
                codebase_path.display(),
                project_name
            )
        })?;

        // Scan for existing agent files
        let hits = scan_agent_files(&codebase_path).context("failed to scan existing agent files")?;
        let migrate = resolve_migrate_mode(self.migrate.as_deref(), self.yes, !hits.is_empty())?;

        if !hits.is_empty() {
            println!("Found {} existing agent file/folder entries.", hits.len());

            // Extract conventions/notes from legacy files and merge into registry
            let (legacy_conventions, legacy_notes, legacy_tasks) = extract_legacy_hints(&hits);
            for convention in legacy_conventions {
                if !codebase.conventions.iter().any(|c| c == &convention) {
                    codebase.conventions.push(convention);
                }
            }
            for note in legacy_notes {
                if !codebase.notes.iter().any(|n| n == &note) {
                    codebase.notes.push(note);
                }
            }
            merge_legacy_tasks(&mut codebase, &legacy_tasks);
            registry::save_codebase_at(&home, &project_key, &codebase)
                .context("failed saving merged legacy hints to registry")?;

            // Backup all discovered agent files
            let backup_items: Vec<BackupItem> = hits
                .iter()
                .map(|hit| BackupItem {
                    provider: hit.provider.clone(),
                    path: hit.path.clone(),
                    is_subagent: hit.is_subagent,
                })
                .collect();

            let manifest = backup_agent_files(&codebase_path, &backup_items)
                .context("failed while backing up existing agent files")?;

            println!(
                "Backed up {} file entries to {}",
                manifest.files.len(),
                backup_dir(&codebase_path).display()
            );

            // Deletion is optional and happens only after a successful sync + import pass.
        }

        // Create orchestra/.gitignore to keep backups out of version control.
        let gitignore_path = orchestra_dir(&codebase_path).join(".gitignore");
        if !gitignore_path.exists() {
            let _ = std::fs::create_dir_all(orchestra_dir(&codebase_path));
            let _ = std::fs::write(&gitignore_path, "backup/\n");
        }

        // Run the sync pipeline
        let mut results = pipeline::run(&home, SyncScope::Codebase(codebase.name.0.clone()), false)
            .with_context(|| format!("sync failed for '{}'", codebase.name))?;

        if let Some(result) = results.pop() {
            let changed = result
                .writes
                .iter()
                .filter(|w| !matches!(w, orchestra_sync::WriteResult::Unchanged { .. }))
                .count();
            println!("Synced '{}' ({} file updates).", result.codebase_name, changed);
        }

        if !hits.is_empty() {
            let imported = import_existing_agent_files(&codebase_path, &hits)
                .context("failed to import existing agent files into orchestra/controls")?;
            println!(
                "Imported {} existing file entries into {}",
                imported,
                control_dir(&codebase_path).display()
            );

            if self.delete {
                let deleted = delete_original_agent_files(&hits)
                    .context("failed to delete legacy agent files after onboarding")?;
                println!("Deleted {} original agent file/folder entries.", deleted);
                println!("  orchestra/pilot.md is now the single source of truth for agent direction.");
            }
        }

        println!(
            "✓ Onboarded '{}' under project '{}'.",
            codebase.name, project_name
        );
        println!("  Pilot entrypoint: {}", pilot_path(&codebase.path).display());
        println!("  Control folder: {}", control_dir(&codebase.path).display());

        // Present migration options to the user
        if !hits.is_empty() {
            println!();
            match migrate {
                MigrateMode::Prompt => {
                    print_migration_prompt(&codebase_path, &hits, &codebase.name.0, self.delete);
                }
                MigrateMode::Mechanical => {
                    println!("────────────────────────────────────────────────────────────");
                    println!("  Mechanical migration complete.");
                    println!();
                    println!("  Your existing agent files were backed up and imported into {}.", control_dir(&codebase_path).display());
                    println!("  Orchestra-generated control files now live in that same tree.");
                    if self.delete {
                        println!("  Legacy agent files were deleted after import because --delete was set.");
                    } else {
                        println!("  Legacy agent files were preserved in-place because --delete was not set.");
                    }
                    println!();
                    println!("  Conventions and notes discovered in the original files were");
                    println!("  merged into the registry and will appear in future syncs.");
                    println!("────────────────────────────────────────────────────────────");
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Migration prompt generator
// ---------------------------------------------------------------------------

fn print_migration_prompt(
    codebase_path: &std::path::Path,
    hits: &[AgentFileHit],
    codebase_name: &str,
    delete_originals: bool,
) {
    // Build the list of discovered files
    let file_list: Vec<String> = hits
        .iter()
        .map(|h| {
            let rel = h.path
                .strip_prefix(codebase_path)
                .unwrap_or(&h.path)
                .display()
                .to_string();
            format!("  - [{}] {}", h.provider, rel)
        })
        .collect();

    let file_list_str = file_list.join("\n");
    let pilot_path = pilot_path(codebase_path).display().to_string();
    let guide_path = guide_path(codebase_path).display().to_string();
    let backup_path = backup_dir(codebase_path).display().to_string();
    let control_path = control_dir(codebase_path).display().to_string();
    let source_material_note = if delete_originals {
        format!(
            "These files were backed up to `{backup_path}` and imported into the generated Orchestra destinations. Because `--delete` was used, the legacy originals may already be removed from the repo."
        )
    } else {
        format!(
            "These files are still in place in the repo, backed up to `{backup_path}`, and also represented inside `{control_path}` for safe review."
        )
    };
    let source_reconciliation_rule = if delete_originals {
        "5. **Do NOT recreate the legacy folders as source of truth.** If there is uncertainty, use the backup plus the imported material already placed under the generated Orchestra files. `orchestra/pilot.md` remains the master direction file.".to_string()
    } else {
        "5. **Do NOT delete source material while reconciling.** Originals are still in the repo and backed up. If there is uncertainty, preserve the original text inside the generated Orchestra files and explain what still needs manual review.".to_string()
    };
    let prompt = format!(
r#"────────────────────────────────────────────────────────────
  🎼 ORCHESTRA SETUP PROMPT (paste this into your agent chat)
────────────────────────────────────────────────────────────

---

# Orchestra One-Shot Setup for `{codebase_name}`

You are setting up Orchestra — the control harness that keeps all agent files,
rules, subagents, skills, and task context in sync across Claude, Cursor,
Copilot, Codex, Windsurf, Gemini, Cline, and Antigravity. Your goal is to
finish onboarding without losing any user content or context.

## What Orchestra did

Orchestra successfully onboarded this codebase and:
1. Detected the tech stack
2. Backed up all existing agent files to: `{backup_path}`
3. Generated managed control files under: `{control_path}`
4. Imported existing agent files into the matching generated Orchestra destinations under `{control_path}`
    and preserved conflicting content inline or as adjacent imported files when a direct merge was not possible
5. Created `{pilot_path}` — the universal master entry point
6. Created `{guide_path}` — the hidden durable context guide

## The original user files Orchestra detected

{source_material_note}

{file_list_str}

## Your task

1. **Read `{pilot_path}` first.** This is the master orchestrator file.
    Treat it as the control-plane entry point for this codebase.
    Then read `{guide_path}` for durable repository context and preserved guidance.
    After that, inspect the rest of `{control_path}` in the background
    so you understand how Orchestra expects agents, skills, rules, and task
    handoffs to work before making changes.

2. **Review the imported originals in `{control_path}`.** For each provider:
   - Look up the **official documentation** for that agent's file format:
     - Claude: https://docs.anthropic.com/en/docs/claude-code/memory
     - Cursor: https://docs.cursor.com/context/rules-for-ai
     - Windsurf: https://docs.windsurf.com/windsurf/memories
     - GitHub Copilot: https://docs.github.com/en/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot
     - Codex (OpenAI): https://platform.openai.com/docs/codex
     - Gemini: https://developers.google.com/gemini-api/
     - Cline: https://github.com/cline/cline
     - Antigravity: https://docs.antigravity.dev/
     - Identify all custom content: skills, subagent definitions, rules,
         custom instructions, conventions, memory entries, persona definitions,
         task-routing notes, and workflow rules.
     - Reconcile that content into the best matching files under `{control_path}`
         without dropping anything.
     - Treat the canonical `<!-- orchestra:tasks -->` block rendered in managed files as the
         editable cross-provider task surface. Editing that block in any managed file should
         sync the same task state into every other managed provider file after save.

3. **Reconcile content into Orchestra control files carefully:**
    - Keep `{pilot_path}` as the overall master file.
    - Keep `{guide_path}` as the hidden background-context document.
     - Keep the generated files in `{control_path}` as the active managed files.
     - Where Orchestra already imported adjacent files or inline legacy blocks,
         use them as the starting point.
     - Preserve names and wording where possible so the user's existing setup is
         still recognizable.
     - If a provider requires extra files or nested directories, create them under
         `{control_path}` rather than in the repo root.

4. **Verify pilot is master and control is authoritative:**
     - Every managed agent file should point back to `{pilot_path}` first.
     - `pilot.md` should describe when to delegate to subagents, when to inspect
         skills and rules, and how Orchestra keeps the control files synchronized.
     - The managed task block should stay structurally valid: `| ID | Title | Status | Description |`.
     - Use task status `done` to close work instead of deleting rows.
     - The control tree under `{control_path}` should be the single place an agent
         inspects before planning task work.

{source_reconciliation_rule}

6. **Test the setup** by running:
   ```
   orchestra status
   orchestra doctor
   ```

## Key principle

Orchestra is the harness — it syncs context, task state, delegation, and
writeback across the control folder. The user's existing skills, subagents,
rules, and instructions are the domain expertise. `pilot.md` is the master.

---"#,
        codebase_name = codebase_name,
        backup_path = backup_path,
        pilot_path = pilot_path,
        guide_path = guide_path,
        control_path = control_path,
        source_material_note = source_material_note,
        source_reconciliation_rule = source_reconciliation_rule,
        file_list_str = file_list_str,
    );

    println!("{}", prompt);
    println!();
    println!("  Copy everything between the --- lines above and paste it into");
    println!("  your agent chat for a one-shot guided setup.");
    println!();
    println!("  Or run with --migrate mechanical to keep everything under orchestra/controls and let Orchestra handle the rest mechanically.");
    println!("────────────────────────────────────────────────────────────");
}

fn resolve_migrate_mode(
    provided: Option<&str>,
    auto_yes: bool,
    has_existing_files: bool,
) -> Result<MigrateMode> {
    if let Some(mode) = provided {
        return mode.parse().map_err(|e: String| anyhow::anyhow!(e));
    }

    if !has_existing_files || auto_yes {
        return Ok(MigrateMode::Prompt);
    }

    println!("Choose how Orchestra should preserve your existing agent setup:");
    println!("  1. Prompt-assisted setup (recommended)");
    println!("     Generates a one-shot prompt for your coding agent to reconcile");
    println!("     the imported orchestra/controls files using");
    println!("     each provider's official documentation.");
    println!("  2. Mechanical copy");
    println!("     Keeps everything inside orchestra/controls and lets Orchestra rely on");
    println!("     the detected conventions/notes without agent-guided reconciliation.");

    loop {
        let input = prompt("Migration mode [1/2, default 1]: ")?;
        match input.trim() {
            "" | "1" => return Ok(MigrateMode::Prompt),
            "2" => return Ok(MigrateMode::Mechanical),
            value if value.eq_ignore_ascii_case("prompt") => return Ok(MigrateMode::Prompt),
            value if value.eq_ignore_ascii_case("mechanical") => {
                return Ok(MigrateMode::Mechanical)
            }
            _ => println!("Please choose 1 (recommended) or 2."),
        }
    }
}

fn import_existing_agent_files(
    codebase_path: &std::path::Path,
    hits: &[AgentFileHit],
) -> Result<usize> {
    cleanup_legacy_import_artifacts(codebase_path, hits)?;

    let mut imported = 0usize;
    for hit in hits {
        if !hit.path.exists() {
            continue;
        }

        imported += import_path_into_controls(codebase_path, &hit.path)?;
    }

    Ok(imported)
}

fn cleanup_legacy_import_artifacts(
    codebase_path: &std::path::Path,
    hits: &[AgentFileHit],
) -> Result<()> {
    for hit in hits {
        let relative = hit.path.strip_prefix(codebase_path).unwrap_or(&hit.path);
        let legacy_destination = control_dir(codebase_path).join(relative);
        if !legacy_destination.exists() {
            continue;
        }

        let new_destination = match resolve_import_destination(relative) {
            ImportDestination::Control(path) => control_dir(codebase_path).join(path),
            ImportDestination::Guide => guide_path(codebase_path),
        };

        if legacy_destination == new_destination {
            continue;
        }

        if legacy_destination.is_dir() {
            fs::remove_dir_all(&legacy_destination).with_context(|| {
                format!("failed to remove legacy import mirror {}", legacy_destination.display())
            })?;
        } else {
            fs::remove_file(&legacy_destination).with_context(|| {
                format!("failed to remove legacy import mirror {}", legacy_destination.display())
            })?;
        }
    }

    Ok(())
}

fn import_path_into_controls(codebase_path: &std::path::Path, source: &std::path::Path) -> Result<usize> {
    if source.is_dir() {
        let mut imported = 0usize;
        for entry in fs::read_dir(source)
            .with_context(|| format!("failed to read {}", source.display()))?
        {
            let entry = entry.with_context(|| format!("failed to read {}", source.display()))?;
            imported += import_path_into_controls(codebase_path, &entry.path())?;
        }
        return Ok(imported);
    }

    let relative = source
        .strip_prefix(codebase_path)
        .unwrap_or(source);
    match resolve_import_destination(relative) {
        ImportDestination::Control(destination_relative) => {
            let destination = control_dir(codebase_path).join(destination_relative);
            import_file(source, &destination, relative)?;
        }
        ImportDestination::Guide => {
            let destination = guide_path(codebase_path);
            import_file(source, &destination, relative)?;
        }
    }
    Ok(1)
}

enum ImportDestination {
    Control(PathBuf),
    Guide,
}

fn resolve_import_destination(relative: &std::path::Path) -> ImportDestination {
    let stripped = strip_leading_component(relative, "AGENT").unwrap_or_else(|| relative.to_path_buf());

    if should_import_into_guide(&stripped) {
        return ImportDestination::Guide;
    }

    ImportDestination::Control(normalize_import_relative_path(&stripped))
}

fn should_import_into_guide(relative: &std::path::Path) -> bool {
    if relative.components().count() != 1 {
        return false;
    }

    let Some(name) = relative.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    let reserved = [
        "CLAUDE.md",
        "CLAUDE.local.md",
        "AGENTS.md",
        "GEMINI.md",
        "ANTIGRAVITY.md",
        ".cursorrules",
        ".windsurfrules",
        ".clinerules",
    ];

    !reserved.contains(&name) && is_text_mergeable(relative)
}

fn normalize_import_relative_path(relative: &std::path::Path) -> PathBuf {
    if relative == std::path::Path::new("CLAUDE.md")
        || relative == std::path::Path::new("CLAUDE.local.md")
        || relative == std::path::Path::new(".claude/CLAUDE.md")
    {
        return PathBuf::from("CLAUDE.md");
    }

    if relative == std::path::Path::new("AGENTS.md") {
        return PathBuf::from("AGENTS.md");
    }

    if relative == std::path::Path::new("GEMINI.md") {
        return PathBuf::from("GEMINI.md");
    }

    if relative == std::path::Path::new("ANTIGRAVITY.md") {
        return PathBuf::from(".agent/rules/orchestra.md");
    }

    if relative == std::path::Path::new(".cursorrules") {
        return PathBuf::from(".cursor/rules/orchestra.mdc");
    }

    if relative == std::path::Path::new(".windsurfrules") {
        return PathBuf::from(".windsurf/rules/orchestra.md");
    }

    if relative == std::path::Path::new(".clinerules") {
        return PathBuf::from(".clinerules/orchestra.md");
    }

    if let Some(stripped) = strip_leading_component(relative, "cursor") {
        return PathBuf::from(".cursor").join(stripped);
    }

    if let Some(stripped) = strip_leading_component(relative, "windsurf") {
        return PathBuf::from(".windsurf").join(stripped);
    }

    if let Some(stripped) = strip_leading_component(relative, "gemini") {
        return PathBuf::from(".gemini").join(stripped);
    }

    if let Some(stripped) = strip_leading_component(relative, "antigravity") {
        return PathBuf::from(".agent").join(stripped);
    }

    relative.to_path_buf()
}

fn strip_leading_component(path: &std::path::Path, prefix: &str) -> Option<PathBuf> {
    let mut components = path.components();
    let first = components.next()?;
    if first.as_os_str() != std::ffi::OsStr::new(prefix) {
        return None;
    }

    Some(components.collect())
}

fn delete_original_agent_files(hits: &[AgentFileHit]) -> Result<usize> {
    let mut paths: Vec<PathBuf> = hits.iter().map(|hit| hit.path.clone()).collect();
    paths.sort_by(|a, b| {
        b.components()
            .count()
            .cmp(&a.components().count())
            .then_with(|| b.cmp(a))
    });
    paths.dedup();

    let mut deleted = 0usize;
    for path in paths {
        if !path.exists() {
            continue;
        }

        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        deleted += 1;
    }

    Ok(deleted)
}

fn import_file(source: &std::path::Path, destination: &std::path::Path, relative: &std::path::Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if !destination.exists() {
        fs::copy(source, destination).with_context(|| {
            format!(
                "failed to import {} into {}",
                source.display(),
                destination.display()
            )
        })?;
        return Ok(());
    }

    if is_text_mergeable(source) {
        merge_text_file(source, destination, relative)?;
    } else {
        let imported_destination = imported_conflict_path(destination);
        if !imported_destination.exists() {
            fs::copy(source, &imported_destination).with_context(|| {
                format!(
                    "failed to preserve conflicting import {} into {}",
                    source.display(),
                    imported_destination.display()
                )
            })?;
        }
    }

    Ok(())
}

fn is_text_mergeable(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()).unwrap_or_default(),
        "md" | "mdc" | "txt" | "yml" | "yaml"
    )
}

fn merge_text_file(source: &std::path::Path, destination: &std::path::Path, relative: &std::path::Path) -> Result<()> {
    let import_id = relative.display().to_string();
    let existing = fs::read_to_string(destination)
        .with_context(|| format!("failed to read {}", destination.display()))?;
    if existing.contains(&format!("{IMPORT_BLOCK_START}{import_id}")) {
        return Ok(());
    }

    let incoming = fs::read_to_string(source)
        .with_context(|| format!("failed to read {}", source.display()))?;
    let trimmed_existing = existing.trim_end();
    let trimmed_incoming = incoming.trim_end();
    let merged = if trimmed_existing.is_empty() {
        format!(
            "{IMPORT_BLOCK_START}{import_id} -->\n{trimmed_incoming}\n{IMPORT_BLOCK_END}\n"
        )
    } else {
        format!(
            "{trimmed_existing}\n\n{IMPORT_BLOCK_START}{import_id} -->\n{trimmed_incoming}\n{IMPORT_BLOCK_END}\n"
        )
    };

    fs::write(destination, merged)
        .with_context(|| format!("failed to write {}", destination.display()))?;
    Ok(())
}

fn imported_conflict_path(destination: &std::path::Path) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = destination
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("imported");
    match destination.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => parent.join(format!("{stem}.imported.{ext}")),
        _ => parent.join(format!("{stem}.imported")),
    }
}

// ---------------------------------------------------------------------------
// Project type resolution
// ---------------------------------------------------------------------------

fn resolve_project_type(codebase_path: &std::path::Path, auto_yes: bool) -> Result<ProjectType> {
    let detected = detect_stack(codebase_path).ok();
    let mut selected = detected
        .as_ref()
        .map(|d| d.project_type.clone())
        .unwrap_or(ProjectType::Backend);

    if let Some(stack) = detected {
        println!(
            "Detected stack: {}{} -> {}",
            stack.primary_language,
            stack
                .framework
                .as_ref()
                .map(|f| format!(" / {f}"))
                .unwrap_or_default(),
            stack.project_type
        );
    } else {
        println!("Could not detect stack confidently; defaulting to backend.");
    }

    if auto_yes {
        return Ok(selected);
    }

    let confirm = prompt("Use this project type? [Y/n/change]: ")?;
    let confirm = confirm.trim().to_ascii_lowercase();
    if confirm == "n" || confirm == "no" || confirm == "change" || confirm == "c" {
        selected = prompt_project_type()?;
    }

    Ok(selected)
}

fn prompt_project_type() -> Result<ProjectType> {
    loop {
        let input = prompt("Project type (backend|frontend|mobile|ml): ")?;
        match input.trim().to_ascii_lowercase().as_str() {
            "backend" => return Ok(ProjectType::Backend),
            "frontend" => return Ok(ProjectType::Frontend),
            "mobile" => return Ok(ProjectType::Mobile),
            "ml" => return Ok(ProjectType::Ml),
            _ => println!("Please enter one of: backend, frontend, mobile, ml"),
        }
    }
}

fn resolve_project_name(
    provided: Option<String>,
    home: &std::path::Path,
    codebase_path: &std::path::Path,
) -> Result<String> {
    if let Some(name) = provided {
        return Ok(name);
    }

    let existing = registry::list_project_names_at(home).context("failed to list project names")?;
    let default_name = codebase_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    if existing.is_empty() {
        let input = prompt(&format!("Project name [{}]: ", default_name))?;
        let trimmed = input.trim();
        return Ok(if trimmed.is_empty() {
            default_name
        } else {
            trimmed.to_string()
        });
    }

    println!(
        "Existing projects: {}",
        existing
            .iter()
            .map(|p| p.0.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Enter an existing project name to attach, or a new name to create one.");

    loop {
        let input = prompt(&format!("Project name [{}]: ", default_name))?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default_name.clone());
        }
        return Ok(trimmed.to_string());
    }
}

fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush().context("failed to flush stdout")?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read user input")?;
    Ok(input)
}

// ---------------------------------------------------------------------------
// Legacy hint extraction
// ---------------------------------------------------------------------------

fn extract_legacy_hints(hits: &[AgentFileHit]) -> (Vec<String>, Vec<String>, Vec<LegacyTaskCandidate>) {
    let mut conventions = Vec::new();
    let mut notes = Vec::new();
    let mut tasks = Vec::new();

    for hit in hits {
        if hit.path.is_file() {
            collect_hints_from_file(&hit.path, &mut conventions, &mut notes, &mut tasks);
            continue;
        }
        if hit.path.is_dir() {
            collect_hints_from_dir(&hit.path, &mut conventions, &mut notes, &mut tasks);
        }
    }

    conventions.sort();
    conventions.dedup();
    notes.sort();
    notes.dedup();

    conventions.truncate(20);
    notes.truncate(20);
    tasks.sort_by(|a, b| a.title.cmp(&b.title));
    tasks.dedup_by(|a, b| a.title.eq_ignore_ascii_case(&b.title));
    tasks.truncate(50);

    (conventions, notes, tasks)
}

#[derive(Debug, Clone)]
struct LegacyTaskCandidate {
    title: String,
    status: TaskStatus,
}

fn collect_hints_from_dir(
    path: &std::path::Path,
    conventions: &mut Vec<String>,
    notes: &mut Vec<String>,
    tasks: &mut Vec<LegacyTaskCandidate>,
) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_hints_from_dir(&p, conventions, notes, tasks);
        } else {
            collect_hints_from_file(&p, conventions, notes, tasks);
        }
    }
}

fn collect_hints_from_file(
    path: &std::path::Path,
    conventions: &mut Vec<String>,
    notes: &mut Vec<String>,
    tasks: &mut Vec<LegacyTaskCandidate>,
) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !matches!(ext, "md" | "mdc" | "txt" | "json" | "yaml" | "yml") {
        return;
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    for line in content.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let lowered = line.to_ascii_lowercase();
        if line.starts_with("-")
            && (lowered.contains("always")
                || lowered.contains("never")
                || lowered.contains("must")
                || lowered.contains("do not")
                || lowered.contains("don't"))
        {
            conventions.push(line.trim_start_matches('-').trim().to_string());
            continue;
        }

        if line.starts_with('#') || line.starts_with("note") || line.starts_with("tip") {
            notes.push(format!("[legacy:{}] {}", path.display(), line));
        }

        if let Some(task) = parse_legacy_task_line(line) {
            tasks.push(task);
        }
    }
}

fn parse_legacy_task_line(line: &str) -> Option<LegacyTaskCandidate> {
    let trimmed = line.trim();

    if let Some(title) = trimmed.strip_prefix("- [ ]") {
        return Some(LegacyTaskCandidate {
            title: title.trim().to_string(),
            status: TaskStatus::Pending,
        });
    }

    if let Some(title) = trimmed.strip_prefix("- [x]").or_else(|| trimmed.strip_prefix("- [X]")) {
        return Some(LegacyTaskCandidate {
            title: title.trim().to_string(),
            status: TaskStatus::Done,
        });
    }

    if let Some(title) = trimmed.strip_prefix("TODO:").or_else(|| trimmed.strip_prefix("Todo:")).or_else(|| trimmed.strip_prefix("todo:")) {
        return Some(LegacyTaskCandidate {
            title: title.trim().to_string(),
            status: TaskStatus::Pending,
        });
    }

    None
}

fn merge_legacy_tasks(codebase: &mut orchestra_core::types::Codebase, tasks: &[LegacyTaskCandidate]) {
    if tasks.is_empty() {
        return;
    }

    if codebase.projects.is_empty() {
        return;
    }

    let existing_titles: std::collections::BTreeSet<String> = codebase.projects[0]
        .tasks
        .iter()
        .map(|task| task.title.to_ascii_lowercase())
        .collect();
    let mut next_index = codebase.projects[0].tasks.len() + 1;
    let now = chrono::Utc::now();

    for task in tasks {
        if existing_titles.contains(&task.title.to_ascii_lowercase()) {
            continue;
        }

        codebase.projects[0].tasks.push(Task {
            id: TaskId::from(format!("T-{next_index:03}")),
            title: task.title.clone(),
            status: task.status.clone(),
            description: None,
            subtasks: vec![],
            notes: vec![format!("[{}] imported from legacy agent files during onboarding", now.format("%Y-%m-%dT%H:%M:%SZ"))],
            created_at: now,
            updated_at: now,
        });
        next_index += 1;
    }
}
