---
name: orchestra-template-rendering
description: Implement Phase 02 template engine behavior: Tera rendering, shared partials, per-agent outputs, hash tracking, and dry-run sync.
---

# Objective
Render all agent-specific files from registry state with deterministic output.

# Use This Skill When
- Editing template context, Tera templates, or per-agent renderers.
- Implementing `sync`, `sync --all`, or `sync --dry-run` behavior.
- Debugging hash no-op logic or template override precedence.

# Procedure
1. Build `TemplateContext` from registry data, excluding done tasks.
2. Render per agent type using embedded defaults plus user overrides.
3. Compare SHA-256 against hash store before writing files.
4. Write atomically, preserving unchanged file mtimes on no-op.
5. Keep shared partials for conventions, skills, and tasks reusable.

# Guardrails
- No partial writes.
- Do not overwrite untracked files without explicit force control.
- Keep each agent output aligned to its format constraints.

# Done Criteria
- All target agent files render from one registry.
- Repeated sync with no changes reports no-op.
- `--dry-run` reports exactly what a real sync would write.
