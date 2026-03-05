# Orchestra Re-Architecture Work Log

Status: Completed
Date: 2026-03-05

## Goal

Implement onboarding-first Orchestra UX with:

- `orchestra onboard` (zero/low-friction setup)
- `orchestra doctor` (broader system health)
- robust agent file discovery + backup + cleanup
- `.orchestra/pilot.md` as universal entry point
- sync regeneration so all provider files stay aligned

## Decisions Confirmed

- Detect stack, then confirm project type.
- Backup existing agent files before cleanup.
- Auto-sync after onboarding.
- Doctor includes version/update, daemon, registry integrity, missing paths, pilot presence.

## Implementation Checklist

- [x] Add `onboard` command wiring in CLI
- [x] Add interactive/non-interactive onboarding flow
- [x] Add detector scan for existing agent files/folders
- [x] Add backup/cleanup module and integrate into onboarding
- [x] Add pilot rendering in renderer/sync pipeline
- [x] Add `doctor` command wiring in CLI
- [x] Add doctor checks and output (`--json` support)
- [x] Add tests for onboard/doctor/scanner
- [x] Harden onboarding cleanup with managed-file protection
- [x] Migrate legacy instruction hints into registry memory during onboarding
- [x] Add delegated/worktree writeback ownership hint support
- [x] Expand generated Cline outputs with skill artifact
- [x] Run workspace tests

## Progress Log

- 2026-03-05: Created work log and execution todo list.
- 2026-03-05: Added `scan_agent_files()` in detector with provider-aware path detection.
- 2026-03-05: Added backup/cleanup module in sync (`backup_agent_files`, `remove_agent_files`) with manifest output at `.orchestra/backup/manifest.json`.
- 2026-03-05: Added `pilot.md.tera` and renderer API to render `.orchestra/pilot.md`.
- 2026-03-05: Integrated pilot generation into sync pipeline so every sync regenerates pilot.
- 2026-03-05: Added new CLI commands `onboard` and `doctor` + command wiring and help updates.
- 2026-03-05: Implemented onboarding flow with detect+confirm project type, project selection prompt, backup+cleanup, and auto-sync.
- 2026-03-05: Implemented doctor checks: update availability, PATH/binary, daemon status, registry integrity, missing paths, pilot presence, and staleness summary (`--json` supported).
- 2026-03-05: Added tests: `orchestra-detector/tests/scan_agent_files.rs`, `orchestra-cli/tests/onboard_flow.rs`, and `orchestra-cli/tests/doctor_checks.rs`.
- 2026-03-05: Updated provider templates to reference `.orchestra/pilot.md` for pilot-first context handoff.
- 2026-03-05: Validation complete: `cargo test --workspace` passed.
- 2026-03-05: Expanded renderer output generation for Claude to include `.claude/rules/orchestra.md`, `.claude/agents/orchestra-worker.md`, and `.claude/agents/orchestra-reviewer.md`.
- 2026-03-05: Expanded renderer output generation for Copilot to include `.github/instructions/orchestra.instructions.md` in addition to repo-wide instructions.
- 2026-03-05: Added new renderer tests to validate expanded Claude and Copilot output counts.
- 2026-03-05: Revalidated renderer, sync, and cli suites after multi-file generation changes.
- 2026-03-05: Hardened onboarding cleanup via protected removal (`remove_agent_files_protected`) so managed outputs are preserved while legacy artifacts are pruned.
- 2026-03-05: Added onboarding legacy hint extraction to ingest conventions/notes from discovered agent files into registry memory before resync.
- 2026-03-05: Expanded doctor diagnostics with daemon socket existence and managed-file presence checks per codebase.
- 2026-03-05: Added writeback `codebase_hint: <name>` command to support delegated/worktree-originated updates when path ownership mapping is ambiguous.
- 2026-03-05: Added Cline skill output generation at `.agents/skills/orchestra-sync/skill.md` and coverage test.
- 2026-03-05: Final validation complete: `cargo test --workspace` passed after hardening changes.
- 2026-03-05: Added skill output generation for Cursor, Windsurf, Codex, Gemini, and Antigravity with new provider-specific templates.
- 2026-03-05: Updated renderer mapping/tests for expanded output counts; revalidated with `cargo test -p orchestra-renderer` and `cargo test --workspace`.
