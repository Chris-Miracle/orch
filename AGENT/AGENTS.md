# Orchestra Agent Router

This repository contains cross-platform agent instructions generated from:
- `Orchestra FRD PRD Engineering Phases.pdf` (v1.0, Feb 2026)
- `Orchestra Harness Engineering.pdf` (v1.0, Feb 2026)

## Mission
Implement Orchestra as a deterministic, offline-first multi-agent synchronization system.

## Phase Order (Do Not Reorder)
1. Foundation: registry + CLI skeleton.
2. Template engine: per-agent rendering + atomic writes + hash store.
3. Staleness/visibility: status signals and diff observability.
4. Daemon/watcher: async autosync + launchd integration.
5. Writeback protocol: agent update blocks + propagation + teaching errors.

## Global Constraints
- Registry YAML is source of truth; rendered agent files are outputs.
- Never perform destructive deletes for user-owned files.
- Use atomic writes (`.tmp` + `rename`) for mutating operations.
- Keep output deterministic and reproducible.
- Prefer mechanical enforcement (templates/tests/rules) over advisory prose.

## Active Skill Catalog
- `orchestra-registry-foundation`
- `orchestra-template-rendering`
- `orchestra-staleness-observability`
- `orchestra-daemon-watcher`
- `orchestra-writeback-protocol`
- `orchestra-rust-quality-gates`
- `orchestra-harness-alignment`
- `orchestra-cross-agent-sync`

## Agent Role Catalog
- `orchestra-orchestrator`
- `rust-foundation-engineer`
- `template-sync-engineer`
- `staleness-observability-engineer`
- `daemon-runtime-engineer`
- `writeback-protocol-engineer`
- `qa-reliability-reviewer`
- `harness-context-curator`

## Writeback Command Canon
- `task_completed: <task-id>`
- `task_started: <task-id>`
- `task_blocked: <task-id> | <reason>`
- `subtask_done: <task-id>/<subtask-title>`
- `skill_discovered: <skill-id> | <description>`
- `convention_added: <text>`
- `note: <free text>`
- `subagent_used: <subagent-id>`
- `file_created: <relative/path>`
- `file_deleted: <relative/path>`

## Required Verification
- Build + clippy + tests before completion.
- Call out regression risk and missing tests explicitly.
- Keep `CLAUDE.md` and `AGENTS.md` concise and pointer-first.

