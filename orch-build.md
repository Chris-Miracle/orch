# Orchestra - Phase 03 Build Reference

Complete implementation record for Phase 03 (staleness detection, status visibility, and pre-sync diff observability).

---

## Phase 03 Outcome

- Status: Completed
- New user command surface:
  - `orchestra status [--project <name>] [--json]`
  - `orchestra diff <codebase>`
- New sync observability modules:
  - `orchestra-sync::staleness`
  - `orchestra-sync::diff`
- Expanded sync signal model:
  - `NeverSynced`, `Current`, `Stale`, `Modified`, `Orphan`
- Freshness source of truth:
  - hash-store `synced_at` timestamp + registry YAML mtime

---

## Validation Executed

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --offline
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets --offline -- -D warnings
```

Result:
- All workspace tests passed.
- Clippy passed with `-D warnings`.

---

## Complete File Manifest (Phase 03)

| Path | State | Phase 03 change |
|---|---|---|
| `Cargo.lock` | Modified | Lockfile refreshed for Phase 03 deps (`similar`, `colored`, `tabled`, `filetime`, `assert_cmd`, `predicates`, and transitive crates). |
| `orchestra-cli/Cargo.toml` | Modified | Added runtime deps for status output/JSON (`colored`, `tabled`, `serde`, `serde_json`) and test deps (`assert_cmd`, `predicates`). |
| `orchestra-cli/src/commands/mod.rs` | Modified | Added command modules `status` and `diff`. |
| `orchestra-cli/src/main.rs` | Modified | Registered `Status` and `Diff` in clap tree and command dispatch. |
| `orchestra-sync/Cargo.toml` | Modified | Added `similar` for unified diff generation; added `filetime` dev-dependency for mtime test control. |
| `orchestra-sync/src/lib.rs` | Modified | Exported new modules/APIs: `diff`, `staleness`, `diff_codebase`, `DiffCodebaseResult`, `FileDiff`, `StalenessSignal`. |
| `orchestra-sync/src/writer.rs` | Modified | Added `find_codebase_at`/shared context helpers and persisted `synced_at` as `sync_started_at` on real sync completion. |
| `phases.md` | Modified | Marked Phase 03 complete and documented user-facing commands/state locations. |
| `orchestra-cli/src/commands/diff.rs` | New | Implemented `orchestra diff <codebase>` command with unified diff output. |
| `orchestra-cli/src/commands/status.rs` | New | Implemented `orchestra status` command with table + JSON output and project filtering. |
| `orchestra-cli/tests/status_and_diff.rs` | New | Added end-to-end CLI tests for diff accuracy and status JSON schema/status correctness. |
| `orchestra-sync/src/diff.rs` | New | Implemented in-memory render diff engine with unified diff output and line-ending normalization. |
| `orchestra-sync/src/staleness.rs` | New | Implemented Phase 03 staleness algorithm and signal model with unit tests. |
| `orchestra-sync/tests/phase03_staleness.rs` | New | Added integration tests for Stale/Modified/Current/NeverSynced scenarios using controlled mtimes. |

---

## CLI Surface Added in Phase 03

Implemented in:
- `/Users/chris/Dev/OS/orch/orchestra-cli/src/commands/status.rs`
- `/Users/chris/Dev/OS/orch/orchestra-cli/src/commands/diff.rs`
- `/Users/chris/Dev/OS/orch/orchestra-cli/src/main.rs`

### `orchestra status`

Supported forms:

```bash
orchestra status
orchestra status --project <project>
orchestra status --json
orchestra status --project <project> --json
```

Behavior:
- Resolves home via `dirs::home_dir()`.
- Loads all registered codebases through `registry::list_codebases_at(home)`.
- Optional server-side filter by project name (`--project`).
- Computes per-codebase signal using `orchestra_sync::staleness::check`.
- Includes:
  - codebase name
  - signal + detail
  - last-sync age (`synced_at` from hash store, or `never`)
  - active task count (non-`done` tasks)
- Human mode prints grouped table and colored `■` indicators legend.
- JSON mode prints stable object shape:
  - top-level: `summary`, `codebases`
  - summary: `projects`, `codebases`, `stale`
  - row: `project`, `codebase`, `status`, `detail`, `last_sync_age`, `last_sync_at`, `active_tasks`

Status values emitted:
- `never_synced`
- `current`
- `stale`
- `modified`
- `orphan`

### `orchestra diff`

Supported form:

```bash
orchestra diff <codebase>
```

Behavior:
- Renders all managed agent outputs in memory (no writes).
- Reads current file content from disk (missing files treated as empty).
- Prints unified diffs with `a/<relative>` and `b/<relative>` headers.
- Prints `No differences for '<codebase>'.` when clean.

---

## Sync Library API Delta (Phase 03)

Implemented in `/Users/chris/Dev/OS/orch/orchestra-sync/src/lib.rs`:

- New module exports:
  - `pub mod staleness;`
  - `pub mod diff;`
- New public re-exports:
  - `pub use staleness::StalenessSignal;`
  - `pub use diff::{diff_codebase, DiffCodebaseResult, FileDiff};`

New core callable surfaces:

```rust
orchestra_sync::staleness::check(home, project, codebase)
orchestra_sync::diff_codebase(codebase_name, home)
```

---

## Staleness Engine Details

Implemented in `/Users/chris/Dev/OS/orch/orchestra-sync/src/staleness.rs`.

### Signal model

```rust
enum StalenessSignal {
  NeverSynced,
  Current,
  Stale { reason: String },
  Modified { files: Vec<PathBuf> },
  Orphan { files: Vec<PathBuf> },
}
```

### Evaluation order

1. `NeverSynced`
- Trigger: hash store file missing OR hash store `files` map empty.

2. `Stale`
- Trigger A: one or more managed agent files missing on disk.
- Trigger B: registry YAML mtime is newer than hash store `synced_at`.
  - Registry timestamp source: `~/.orchestra/projects/<project>/<codebase>.yaml`
  - Sync timestamp source: `~/.orchestra/hashes/<codebase>.json` `synced_at`
  - Comparison safety: `duration_since(UNIX_EPOCH).unwrap_or_default()` for filesystem times.

3. `Modified`
- Trigger: managed file exists and stored hash differs from recomputed hash.
- Hashing behavior:
  - always normalize line endings (`\r\n` -> `\n`) before hashing.

4. `Orphan`
- Trigger A: managed file exists but has no hash store entry.
- Trigger B: hash store contains paths outside the managed set that still exist on disk.

5. `Current`
- Trigger: none of the above.

### Managed file set covered

Computed from `AgentKind::all()` output paths for:
- Claude
- Cursor
- Windsurf
- Copilot
- Codex
- Gemini (3 files)
- Cline
- Antigravity

---

## Diff Engine Details

Implemented in `/Users/chris/Dev/OS/orch/orchestra-sync/src/diff.rs`.

Algorithm:
1. Resolve codebase from registry via shared finder.
2. Build render context.
3. Force `ctx.meta.last_synced = None` for diff stability (avoids timestamp-only false positives).
4. Render all agent outputs in memory.
5. Read existing file or default to empty.
6. Normalize both sides to LF.
7. Compare with `similar::TextDiff::from_lines` and emit unified hunks only for changed files.

Output model:
- `DiffCodebaseResult { codebase_name, diffs: Vec<FileDiff> }`
- `FileDiff { path, unified_diff }`

---

## Writer/Sync Interaction Changes

Implemented in `/Users/chris/Dev/OS/orch/orchestra-sync/src/writer.rs`.

Changes:
- Added shared helpers for cross-module reuse:
  - `build_sync_context(...)`
  - `find_codebase_at(...)`
- Set `sync_started_at = Utc::now()` at beginning of real sync.
- Persist `store.synced_at = sync_started_at` only after all writes and before atomic hash-store save.
- Existing atomic write guarantees retained:
  - content hash normalized with LF
  - hash store entry inserted only after successful rename

---

## Status Output Model

Implemented in `/Users/chris/Dev/OS/orch/orchestra-cli/src/commands/status.rs`.

Terminal mode:
- Header:
  - `Orchestra v<version> | <projects> projects | <codebases> codebases | <stale> stale`
- Grouped by project.
- Table columns:
  - `codebase`
  - `status` (label-only; unicode width-safe)
  - `detail`
  - `last sync`
  - `active tasks`
- Colored `■` indicator legend printed outside table.

JSON mode:
- Deterministic schema and status key strings.
- Intended for scripts/CI parsing.

---

## Test Coverage Added in Phase 03

### 1) Sync integration: explicit Phase 03 staleness scenarios

File:
- `/Users/chris/Dev/OS/orch/orchestra-sync/tests/phase03_staleness.rs`

Cases:
- `stale_when_registry_is_newer_than_managed_files`
  - Uses `filetime` to force old managed-file mtimes and newer registry mtime.
- `modified_when_hash_mismatch_detected`
  - Sync, mutate `CLAUDE.md`, assert `Modified` and file list includes `CLAUDE.md`.
- `current_immediately_after_sync`
  - Sync then assert `Current`.
- `never_synced_when_registry_exists_but_no_hash_store`
  - Registry-only setup then assert `NeverSynced`.

### 2) CLI integration: status + diff end-to-end

File:
- `/Users/chris/Dev/OS/orch/orchestra-cli/tests/status_and_diff.rs`

Cases:
- `diff_accuracy_on_registry_change`
  - Sync baseline.
  - Mutate registry-backed data with unique sentinel.
  - Run `orchestra diff`.
  - Assert sentinel appears on `+` unified diff line.
  - Assert no unrelated `last_synced` noise appears.
- `status_json_includes_all_codebases_with_expected_staleness_and_schema`
  - Build `current`, `modified`, `stale`, `never_synced` codebases.
  - Run `orchestra status --json`.
  - Assert all codebases present with expected statuses.
  - Assert root/summary/row field sets are exact (schema stability guard).

### 3) Module/unit tests added in core Phase 03 modules

`/Users/chris/Dev/OS/orch/orchestra-sync/src/staleness.rs` includes unit cases for:
- `Current`
- `NeverSynced`
- `Stale` (registry newer / missing managed files)
- `Modified`
- `Orphan`
- age formatting helpers

`/Users/chris/Dev/OS/orch/orchestra-sync/src/diff.rs` includes unit cases for:
- clean sync => no diff
- manual edit => unified diff emitted
- hash-store `synced_at` change only => no diff noise

---

## Dependency and Lockfile Delta (Phase 03)

### Manifest-level additions

- `orchestra-sync`:
  - runtime: `similar = "2"`
  - dev: `filetime = "0.2"`

- `orchestra-cli`:
  - runtime: `colored = "2"`, `tabled = "0.14"`, `serde`, `serde_json`
  - dev: `assert_cmd = "2"`, `predicates = "3"`

### Notable lockfile additions

- Runtime-facing: `similar`, `colored`, `tabled`, `tabled_derive`, `papergrid`, `unicode-width`
- Test-facing: `assert_cmd`, `filetime`, `wait-timeout`, `predicates`
- Platform/transitive support: `redox_syscall` and related graph updates

---

## User Quick Reference (Now Available)

```bash
# Table status across all codebases
orchestra status

# Filter status to one project
orchestra status --project copnow

# Parseable machine output
orchestra status --json

# Show pre-sync unified diff for one codebase
orchestra diff copnow_api

# Apply updates after reviewing status/diff
orchestra sync --all
```

Primary state files involved:

```text
~/.orchestra/projects/<project>/<codebase>.yaml
~/.orchestra/hashes/<codebase>.json
```

---

## Phase 03 Completion Notes

- Registry freshness now keys off hash-store `synced_at` instead of rendered file mtimes.
- Hash comparison remains line-ending normalized and deterministic.
- Diff is stable against `last_synced` metadata-only changes.
- Status distinguishes first-run/unsynced (`NeverSynced`) from true staleness (`Stale`).
