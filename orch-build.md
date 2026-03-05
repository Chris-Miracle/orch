# Orchestra - Phase 05 Build Reference

Complete implementation record for Phase 05 (writeback protocol parsing, registry mutation, block stripping/error feedback, daemon propagation, observability, and coverage expansion).

---

## Phase 05 Outcome

- Status: Completed
- User-visible behavior added:
  - Orchestra now consumes agent-authored writeback blocks from managed files.
  - Valid writeback commands mutate registry state and are propagated by sync.
  - Invalid writeback commands produce in-file `orchestra:error` remediation guidance.
  - Daemon watcher now detects managed agent file edits and processes writeback automatically.
- No new top-level CLI command was introduced for writeback itself.
- Writeback runs through existing surfaces:
  - Manual: `orchestra sync <codebase>`
  - Automatic: daemon watcher (`orchestra daemon start`)

---

## Writeback Block Contract (Implemented)

### Markers

- Open marker: `<!-- orchestra:update -->`
- Close marker: `<!-- /orchestra:update -->`
- A block is considered present only when both markers exist.

### Supported command grammar (actual parser contract)

- `task_completed|<task_id>`
- `convention_added|<text>`
- `skill_discovered|<id>|<description>`
- `note|<text>`

### Parser behavior

- Blank lines are ignored.
- Lines starting with `#` are ignored as comments.
- Parsing is best-effort:
  - Valid lines are parsed into commands.
  - Invalid lines are collected as parse errors (line-numbered, raw line retained).
- Unknown verbs return explicit supported-syntax hints.

---

## End-to-End Writeback Flow (Implemented)

Implemented in `orchestra-sync/src/writeback/mod.rs` via `process_writeback(home, agent_file)`.

1. Read target managed agent file.
2. Fast-exit if no complete update block (`WritebackOutcome::no_block()`).
3. Extract + parse block body into `{commands, parse_errors}`.
4. Resolve owning codebase by canonical-path matching against all managed output paths (`AgentKind::output_paths`).
5. Apply valid parsed commands to in-memory codebase model.
6. Persist codebase YAML back to registry.
7. Strip `orchestra:update` block atomically from file.
8. If parse errors exist and strip succeeded, write `orchestra:error` block before update location.
9. Re-run sync pipeline for that codebase (`pipeline::run(..., SyncScope::Codebase(...), false)`).
10. Append writeback event to sync-events log.

Special ownership-fallback behavior:

- If file cannot be mapped to a registered codebase:
  - Block is still stripped.
  - Parse errors are returned in outcome.
  - Registry mutation is skipped.
  - Error block is not written in this branch.

---

## Command Application Semantics (Implemented)

Implemented in `orchestra-sync/src/writeback/applier.rs`.

### `task_completed|<task_id>`

- Searches tasks across all projects in the codebase.
- On match:
  - sets `TaskStatus::Done`
  - updates task `updated_at`
- On no match:
  - emits per-command apply error (`task '<id>' not found in any project`)

### `convention_added|<text>`

- Deduplicates exact text against `codebase.conventions`.
- Duplicate command is reported as `Skipped` with reason.

### `skill_discovered|<id>|<description>`

- Deduplicates by skill `id` against `codebase.skills`.
- Duplicate command is reported as `Skipped` with reason.

### `note|<text>`

- Appends timestamped note string to `codebase.notes`:
  - format: `[YYYY-MM-DDTHH:MM:SSZ] <text>`

### Timestamps

- If at least one command is applied, `codebase.updated_at` is advanced.

---

## Strip + Error Feedback Behavior (Implemented)

Implemented in `orchestra-sync/src/writeback/strip.rs`.

### Strip behavior

- Removes only the update block range between markers.
- Performs atomic write via sibling `.orchestra.tmp` then rename.
- Collapses excess blank lines after block removal.

### Error block behavior

- Error block markers:
  - `<!-- orchestra:error -->`
  - `<!-- /orchestra:error -->`
- Existing prior error block is replaced.
- New error block includes:
  - parse error list (line + raw line + message)
  - supported command syntax list
- Inserted before the update block location.

### Order fix implemented

- Writeback now strips update block first, then writes error block if needed.
- This guarantees update markers are removed even on malformed command input.

---

## Observability + Logging (Implemented)

Implemented in `orchestra-sync/src/writeback/log.rs`.

- Writeback events are appended as NDJSON-style one-object-per-line to:
  - `<home>/.orchestra/logs/sync-events.log`
- Event payload includes:
  - `timestamp`
  - `agent_file`
  - `codebase`
  - `commands_applied`
  - `parse_errors`
  - `apply_errors`
  - `commands`
  - `block_stripped`
  - `error_block_written`

---

## Daemon Integration (Implemented)

Implemented in `orchestra-daemon/src/runtime.rs`.

### Watch scope extension

- In addition to registry tree watch (`~/.orchestra/projects/**`), daemon now watches parent directories of managed agent outputs.
- Managed paths are discovered through `orchestra_sync::managed_agent_paths(...)`.

### Event routing

- Registry YAML create/modify events:
  - existing path (sync enqueue) retained.
- Managed agent file create/modify events:
  - now invoke `process_writeback` in blocking task.

### Path correctness + diagnostics

- Canonical path comparisons added for watcher events and managed file detection.
- Event tracing logs include raw path + canonical path.
- Hash store path alignment diagnostics log mismatches between hash entries and managed watcher paths.

### Own-write suppression

- Added own-write window to reduce self-trigger loops after sync writes.
- Data structure: `HashMap<PathBuf, Instant>`.
- Suppression window: 1 second.
- Managed paths are pre-registered before sync execution.

### Debounce

- Existing debounce behavior retained for rapid events.
- Stale debounce entries are pruned opportunistically.

---

## Core Model + Registry Schema Changes (Phase 05 support)

### `orchestra-core/src/types.rs`

- Added `Skill` struct:
  - `id: String`
  - `description: String`
- Added `Task.notes: Vec<String>` (defaulted/optional in YAML).
- Added codebase-level writeback fields to `Codebase`:
  - `conventions: Vec<String>`
  - `skills: Vec<Skill>`
  - `notes: Vec<String>`
- Added/validated serialization test ensuring `TaskStatus::Done` serializes lowercase (`done`).

### `orchestra-core/src/lib.rs`

- Re-exported `Skill` in public API surface.

### `orchestra-core/src/registry.rs`

- `init_at` and `add_codebase_at` now initialize new writeback fields (`conventions`, `skills`, `notes`) to empty vectors.

---

## Sync Crate Surface Changes

### `orchestra-sync/src/lib.rs`

- Added `pub mod writeback`.
- Re-exported:
  - `managed_agent_paths`
  - `process_writeback`
  - `WritebackOutcome`

### New writeback module files

- `orchestra-sync/src/writeback/types.rs`
- `orchestra-sync/src/writeback/parser.rs`
- `orchestra-sync/src/writeback/applier.rs`
- `orchestra-sync/src/writeback/strip.rs`
- `orchestra-sync/src/writeback/log.rs`
- `orchestra-sync/src/writeback/mod.rs`

---

## Cross-Crate Fixture + Compatibility Updates

These updates were required so all existing tests and serializers compile against the expanded Phase 05 model.

- `orchestra-core/tests/registry_tests.rs`
- `orchestra-core/tests/roundtrip.rs`
- `orchestra-renderer/src/context.rs`
- `orchestra-renderer/src/engine.rs`
- `orchestra-renderer/tests/phase02_template_rendering.rs`
- `orchestra-sync/src/writer.rs`

Each now provides the new required writeback-compatible fields (`Task.notes`, `Codebase.conventions/skills/notes`) in fixtures/builders.

---

## CLI and Integration Test Additions

### `orchestra-cli/Cargo.toml`

- Added dev dependencies needed for Phase 05 integration tests:
  - `tokio-test = "0.4"`
  - `chrono = { version = "0.4", features = ["serde"] }`

### New end-to-end propagation test

- `orchestra-cli/tests/phase05_writeback_propagation.rs`

Test verifies full daemon writeback propagation path:

1. Initialize codebase + task with non-done state.
2. Baseline sync generates managed agent files.
3. Start daemon and wait for running status.
4. Edit `CLAUDE.md` by appending update block with `task_completed|task-A`.
5. Wait until all are true:
   - registry task status becomes `Done`
   - update block removed from source managed file
   - synced downstream managed output reflects updated state
   - sync-events log contains writeback command evidence

---

## Tests Added/Expanded for Phase 05

### Parser tests (`orchestra-sync/src/writeback/parser.rs`)

- Marker detection for complete/incomplete block cases.
- Block extraction boundaries.
- Command parsing success cases.
- Error cases:
  - unknown command verb
  - missing task id
  - malformed skill command
  - colon delimiter typo (`task_completed: T-1`)
  - unsupported formats (`task_blocked`, `subtask_done`)
- Multi-command coverage (5+ commands).
- Mixed valid+invalid line aggregation behavior.

### Applier tests (`orchestra-sync/src/writeback/applier.rs`)

- Task completion success and timestamp advance.
- Missing task id path returns apply error.
- Convention dedup behavior.
- Skill dedup-by-id behavior.
- Timestamped note append behavior.
- Multi-command combined apply behavior.
- Codebase timestamp update behavior.

### Strip/error tests (`orchestra-sync/src/writeback/strip.rs`)

- Block removal preserving surrounding content.
- No-block no-op behavior.
- Block-only content empty result behavior.
- Blank-line normalization behavior.
- Atomic write roundtrip behavior.
- Error block insertion-before-update behavior.
- Existing error block replacement behavior.

### Writeback orchestrator tests (`orchestra-sync/src/writeback/mod.rs`)

- No-block fast exit.
- Successful convention writeback + strip.
- Invalid command path emits error block + strips update block.
- Typo command includes correction suggestion.
- End-to-end task completion path updates registry to `Done`.

### Daemon/runtime tests (`orchestra-daemon/src/runtime.rs`)

- Debounce coalescing behavior.
- Status payload timestamp fields.
- Sync timestamp recording behavior.
- Existing socket/protocol/cache tests retained and passing with Phase 05 integration.

---

## Complete File Manifest (Phase 05)

| Path                                                     | State    | Phase 05 change                                                                                                                              |
| -------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `orchestra-sync/src/writeback/types.rs`                  | Added    | New command/parse/apply/outcome type system for writeback protocol.                                                                          |
| `orchestra-sync/src/writeback/parser.rs`                 | Added    | Marker detection, block extraction, line parser, parse aggregation + tests.                                                                  |
| `orchestra-sync/src/writeback/applier.rs`                | Added    | Command application semantics, dedup, timestamp mutation + tests.                                                                            |
| `orchestra-sync/src/writeback/strip.rs`                  | Added    | Atomic update-block stripping, error-block writing/replacement, blank-line normalization + tests.                                            |
| `orchestra-sync/src/writeback/log.rs`                    | Added    | NDJSON event append logger for sync-events stream + tests.                                                                                   |
| `orchestra-sync/src/writeback/mod.rs`                    | Added    | Full orchestrator (`process_writeback`), owning-codebase detection, managed path enumeration, integration tests.                             |
| `orchestra-sync/src/lib.rs`                              | Modified | Exported writeback module and public writeback APIs.                                                                                         |
| `orchestra-daemon/src/runtime.rs`                        | Modified | Managed-agent watcher registration, writeback event routing, canonical path diagnostics, own-write suppression, hash-path alignment logging. |
| `orchestra-core/src/types.rs`                            | Modified | Added `Skill`, `Task.notes`, `Codebase.conventions/skills/notes`, and status serialization test for `done`.                                  |
| `orchestra-core/src/lib.rs`                              | Modified | Re-exported `Skill`.                                                                                                                         |
| `orchestra-core/src/registry.rs`                         | Modified | Initialize new writeback fields in `init_at` and `add_codebase_at`.                                                                          |
| `orchestra-core/tests/registry_tests.rs`                 | Modified | Fixtures updated for new writeback-compatible fields.                                                                                        |
| `orchestra-core/tests/roundtrip.rs`                      | Modified | Roundtrip fixtures updated for new writeback-compatible fields.                                                                              |
| `orchestra-renderer/src/context.rs`                      | Modified | Context fixtures updated with new Task/Codebase fields.                                                                                      |
| `orchestra-renderer/src/engine.rs`                       | Modified | Engine test fixtures updated with new Codebase fields.                                                                                       |
| `orchestra-renderer/tests/phase02_template_rendering.rs` | Modified | Rendering fixtures updated with new Task/Codebase fields.                                                                                    |
| `orchestra-sync/src/writer.rs`                           | Modified | Writer fixtures updated with new Codebase fields.                                                                                            |
| `orchestra-cli/Cargo.toml`                               | Modified | Added Phase 05 integration-test dev dependencies (`tokio-test`, `chrono`).                                                                   |
| `orchestra-cli/tests/phase05_writeback_propagation.rs`   | Added    | Full daemon-backed end-to-end writeback propagation integration test.                                                                        |

---

## On-Disk Artifacts and Paths Used in Phase 05

- Registry YAML source of truth:
  - `~/.orchestra/projects/<project>/<codebase>.yaml`
- Managed agent files (writeback source/targets):
  - examples: `CLAUDE.md`, `AGENTS.md`, `.cursor/rules/orchestra.mdc`, etc. (from renderer agent output map)
- Writeback event stream:
  - `~/.orchestra/logs/sync-events.log`
- Existing daemon runtime paths still apply:
  - `~/.orchestra/daemon.sock`
  - `~/.orchestra/logs/daemon.log`
  - `~/.orchestra/logs/daemon-err.log`

---

## Verification Performed

Validation that was executed after Phase 05 implementation and fixes:

- Targeted writeback/CLI tests and full workspace test run completed.
- Representative command used:

```bash
cargo test --workspace
```

Observed outcome at completion point:

- Workspace tests passed across `orchestra-core`, `orchestra-sync`, `orchestra-daemon`, `orchestra-cli`, `orchestra-renderer`, and `orchestra-detector`.
- New Phase 05 integration test passed:
  - `phase05_writeback_end_to_end_propagation`

---

## Notes

- This document intentionally records only Phase 05 implementation and its direct compatibility updates.
- Post-Phase-05 unrelated UX/doc/test changes (for example help-text refinements) are excluded from this record.
