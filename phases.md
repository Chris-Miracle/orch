# Orchestra — Phase Reference

---

## Getting Started

Orchestra is a native CLI binary — no pip, no npm, no Node, no Python needed.

### Option 1 — curl installer (no Rust required) ⭐

```bash
curl -fsSL https://raw.githubusercontent.com/chris-miracle/orch/main/install.sh | sh
```

Downloads the pre-built binary for your platform (macOS arm64/x86_64, Linux x86_64) and installs it to `~/.local/bin`. That's it.

### Option 2 — if you have Rust installed

```bash
cargo install --git https://github.com/chris-miracle/orch --bin orchestra
```

No cloning needed. Cargo fetches, compiles, and drops `orchestra` on your PATH.

### Option 3 — build from source

```bash
git clone https://github.com/chris-miracle/orch.git
cd orch
cargo install --path orchestra-cli
```

**Verify**

```bash
orchestra --version
orchestra --help
```

---

## Phase 01 — Foundation ✅

_Registry core, CLI skeleton, stack detector._

### What was implemented

| Area     | Detail                                                              |
| -------- | ------------------------------------------------------------------- |
| Registry | YAML file at `~/.orchestra/registry.yaml` (created on first `init`) |
| CLI      | `orchestra init`, `orchestra project list`, `orchestra project add` |
| Detector | Reads indicator files to auto-detect language + framework           |

### Using it

**Register a codebase from its directory:**

```bash
cd /path/to/your/project
orchestra init . --project myapp --type backend
```

**Or specify the path directly:**

```bash
orchestra init ~/code/myapi --project myapi --type backend
orchestra init ~/code/mobile --project app --type mobile
```

Supported types: `backend` · `frontend` · `mobile` · `ml`

**List everything registered:**

```bash
orchestra project list
```

**Add another project to the first registered codebase:**

```bash
orchestra project add payments --type backend
```

**Where your data lives:**

```
~/.orchestra/registry.yaml   ← single source of truth
```

### Development (without installing)

```bash
# Run without installing
cargo run --bin orchestra -- init . --project myapp --type backend
cargo run --bin orchestra -- project list

# Tests (77 passing)
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
```

---

## Phase 02 — Template Engine ✅

_Per-agent file rendering, hash store, atomic writes._

### What was implemented

| Area         | Detail                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------ |
| Sync command | `orchestra sync <codebase>` and `orchestra sync --all`                                           |
| Rendering    | Agent files generated for Claude, Cursor, Windsurf, Copilot, Codex, Gemini, Cline, Antigravity   |
| Write safety | Atomic writes via `.orchestra.tmp` + rename; unchanged content is skipped by SHA-256 hash        |
| Hash store   | Per-codebase hash file at `~/.orchestra/hashes/<codebase>.json`                                  |
| Library APIs | `orchestra_sync::{sync_codebase, sync_all}` and `orchestra_renderer::{Renderer, TemplateEngine}` |

### Using it

**Sync one registered codebase:**

```bash
orchestra sync copnow_api
```

**Preview without writing files:**

```bash
orchestra sync copnow_api --dry-run
```

**Sync all registered codebases:**

```bash
orchestra sync --all
```

**Where files are written:**

```text
<codebase>/CLAUDE.md
<codebase>/.cursor/rules/orchestra.mdc
<codebase>/.windsurf/rules/orchestra.md
<codebase>/.github/copilot-instructions.md
<codebase>/AGENTS.md
<codebase>/GEMINI.md
<codebase>/.gemini/settings.json
<codebase>/.gemini/styleguide.md
<codebase>/.clinerules/orchestra.md
<codebase>/.agent/rules/orchestra.md
~/.orchestra/hashes/<codebase>.json
```

---

## Phase 03 — Staleness / Observability ✅

_Status signals, diff output, stale-file detection._

### What was implemented

| Area                | Detail                                                                                                      |
| ------------------- | ----------------------------------------------------------------------------------------------------------- |
| Staleness detection | `NeverSynced`, `Current`, `Stale`, `Modified`, `Orphan` signals with hash-store + registry freshness checks |
| Status command      | `orchestra status` with project filtering (`--project`) and machine output (`--json`)                       |
| Diff command        | `orchestra diff <codebase>` renders in-memory and prints unified diffs without writing files                |
| Library APIs        | `orchestra_sync::staleness::check`, `orchestra_sync::StalenessSignal`, `orchestra_sync::diff_codebase`      |
| Data sources        | Reuses `~/.orchestra/projects/<project>/<codebase>.yaml` + `~/.orchestra/hashes/<codebase>.json`            |

### Using it

**See staleness for all codebases:**

```bash
orchestra status
```

**Filter by project:**

```bash
orchestra status --project copnow
```

**Get JSON for scripts/CI:**

```bash
orchestra status --json
```

**Preview changes before syncing:**

```bash
orchestra diff copnow_api
```

**Refresh stale/modified codebases:**

```bash
orchestra sync --all
```

**Where freshness state lives:**

```text
~/.orchestra/projects/<project>/<codebase>.yaml
~/.orchestra/hashes/<codebase>.json
```

---

## Phase 04 — Daemon / Watcher ✅

_Background autosync, launchd integration, file watching._

### What was implemented

| Area                | Detail                                                                                                                                                   |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Daemon runtime      | New `orchestra-daemon` crate with Tokio tasks for watcher, sync processor, and Unix socket server                                                        |
| Watcher behavior    | Watches `~/.orchestra/projects/` registry tree (directory-based FSEvents, debounced) and triggers sync on YAML create/modify                             |
| CLI commands        | `orchestra daemon start`, `stop`, `status`, `install`, `uninstall`, `logs`                                                                               |
| Socket protocol     | Newline-delimited JSON over Unix socket: `{"cmd":"status"}`, `{"cmd":"sync","codebase":"..."}`, `{"cmd":"stop"}`                                         |
| launchd integration | Programmatic plist generation + install/uninstall for `dev.orchestra.daemon`                                                                             |
| Library APIs        | `orchestra_daemon::{start_blocking, request_status, request_stop, request_sync, install_launchd, uninstall_launchd}` and `orchestra_sync::pipeline::run` |

### Using it

**Run daemon in foreground:**

```bash
orchestra daemon start
```

**Check daemon status (JSON):**

```bash
orchestra daemon status
```

**Request a graceful daemon stop:**

```bash
orchestra daemon stop
```

**Install/uninstall launchd service (macOS):**

```bash
orchestra daemon install
orchestra daemon uninstall
```

**Read daemon logs:**

```bash
orchestra daemon logs
orchestra daemon logs --stderr-only --lines 200
```

**Where daemon state lives:**

```text
~/.orchestra/projects/                           ← watched registry tree
~/.orchestra/daemon.sock                         ← Unix socket
~/.orchestra/logs/daemon.log
~/.orchestra/logs/daemon-err.log
~/Library/LaunchAgents/dev.orchestra.daemon.plist
```

---

## Phase 05 — Writeback Protocol ✅

_Agents write back task completions; Orchestra propagates them._

### What was implemented

| Area                   | Detail                                                                                                                            |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ----------------- | -------------------------- | ---------- | --------------------------------- | ------- |
| Writeback block parser | Reads `<!-- orchestra:update --> ... <!-- /orchestra:update -->` blocks from managed agent files during sync/writeback processing |
| Supported commands     | `task.completed                                                                                                                   | <task_id>`, `note | <text>`, `skill.discovered | <skill_id> | <description>`, `convention.added | <text>` |
| Registry updates       | Applies command effects to `tasks`, `notes`, `skills`, and `conventions` in per-codebase YAML                                     |
| Safety + feedback      | Invalid commands are stripped and replaced with an `orchestra:error` block in the file so agents get actionable feedback          |
| Daemon propagation     | With daemon running, writeback changes are auto-detected and propagated through the normal sync pipeline                          |
| Observability/API      | Sync events logged as NDJSON; writeback APIs exported under `orchestra_sync::writeback`                                           |

### Using it

**1) Add a writeback block to a managed file (example: `AGENTS.md`):**

```bash
cat >> AGENTS.md <<'EOF'
<!-- orchestra:update -->
task.completed|phase05-writeback
note|Validated writeback end-to-end.
skill.discovered|writeback-protocol|Can parse/apply orchestra:update blocks safely.
convention.added|Always keep writeback commands pipe-delimited.
<!-- /orchestra:update -->
EOF
```

**2) Apply it now (manual):**

```bash
orchestra sync <codebase>
```

**3) Or let daemon auto-apply changes:**

```bash
orchestra daemon start
orchestra daemon status
```

**4) Verify registry + logs:**

```bash
orchestra status --json
orchestra daemon logs --lines 200
```

**Where writeback state lives:**

```text
~/.orchestra/projects/<project>/<codebase>.yaml
~/.orchestra/logs/sync-events.ndjson
```
