# Orchestra

**The agent abstraction layer for AI coding tools.**

Orchestra is an open-source macOS CLI that manages every AI coding agent — Claude, Copilot, Cursor, Codex, Windsurf, Gemini, Cline, and Antigravity — across all your projects from a single source of truth. It detects your stack, discovers existing agent files, backs them up safely, then renders official-spec instruction files, subagent configs, and skill artifacts for every provider. One registry, one sync, every agent stays current.

[![Release](https://img.shields.io/badge/release-repository%20hosted-informational)](#installation)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/platform-macOS-lightgrey?logo=apple)](#requirements)

---

## Table of Contents

- [What it does](#what-it-does)
- [Supported agents](#supported-agents)
- [Requirements](#requirements)
- [Installation](#installation)
  - [Stable release (recommended)](#stable-release-recommended)
  - [Beta pre-release](#beta-pre-release)
  - [Manual download](#manual-download)
  - [Build from source](#build-from-source)
- [Release channels](#release-channels)
- [Quick start](#quick-start)
- [Operational guide](#operational-guide)
- [Issues and contribution ideas](#issues-and-contribution-ideas)
- [Commands](#commands)
  - [orchestra onboard](#orchestra-onboard)
  - [orchestra offboard](#orchestra-offboard)
  - [orchestra init](#orchestra-init)
  - [orchestra project](#orchestra-project)
  - [orchestra sync](#orchestra-sync)
  - [orchestra status](#orchestra-status)
  - [orchestra diff](#orchestra-diff)
  - [orchestra doctor](#orchestra-doctor)
  - [orchestra daemon](#orchestra-daemon)
  - [orchestra update](#orchestra-update)
  - [orchestra reset](#orchestra-reset)
- [Pilot entry point](#pilot-entry-point)
- [Generated files](#generated-files)
- [Writeback protocol](#writeback-protocol)
- [Registry layout](#registry-layout)
- [Architecture](#architecture)
- [Contributing](#contributing)
- [License](#license)

---

## What it does

1. **Detects** your stack — language, framework, and project type — by inspecting manifest files.
2. **Discovers** existing agent files and libraries (`CLAUDE.md`, `.cursor/rules/`, `.github/copilot-instructions.md`, repo-level `AGENT/`, etc.) and extracts conventions and notes from them.
3. **Backs up** those files to `orchestra/backup/` with a JSON manifest before touching anything.
4. **Imports** existing agent files into their matching generated destinations inside `orchestra/controls/`, merging preserved content into managed files when paths overlap and routing generic legacy guidance into `orchestra/.guide.md`.
5. **Renders** Orchestra's own managed instruction files, subagent definitions, and skill artifacts into `orchestra/controls/` from your single registry YAML.
6. **Generates** `orchestra/pilot.md` as the universal entry point that every agent reads first, plus `orchestra/.guide.md` as durable hidden repo context.
7. **Watches** for changes via a background daemon and auto-syncs when your registry or templates change.
8. **Writes back** — agents can report task updates, convention additions, and status changes directly inside managed files; Orchestra parses and applies them to the registry automatically.

---

## Supported agents

Orchestra generates provider-specific files inside `orchestra/controls/`, keeping the repo root clean while preserving each provider's expected relative structure:

| Agent           | Output files                                                                                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------------- |
| **Claude**      | `CLAUDE.md`, `.claude/rules/orchestra.md`, `.claude/agents/orchestra-worker.md`, `.claude/agents/orchestra-reviewer.md` |
| **Cursor**      | `.cursor/rules/orchestra.mdc`, `.cursor/skills/orchestra-sync/skill.md`                                                 |
| **Windsurf**    | `.windsurf/rules/orchestra.md`, `.windsurf/skills/orchestra-sync/skill.md`                                              |
| **Copilot**     | `.github/copilot-instructions.md`, `.github/instructions/orchestra.instructions.md`                                     |
| **Codex**       | `AGENTS.md`, `.codex/skills/orchestra-sync/skill.md`                                                                    |
| **Gemini**      | `GEMINI.md`, `.gemini/settings.json`, `.gemini/styleguide.md`, `.gemini/skills/orchestra-sync/skill.md`                 |
| **Cline**       | `.clinerules/orchestra.md`, `.agents/skills/orchestra-sync/skill.md`                                                    |
| **Antigravity** | `.agent/rules/orchestra.md`, `.agent/skills/orchestra-sync/skill.md`                                                    |

Every codebase also gets the universal entry point at `orchestra/pilot.md` and the hidden context file at `orchestra/.guide.md`.

---

## Requirements

|           |                                |
| --------- | ------------------------------ |
| **OS**    | macOS (Apple Silicon or Intel) |
| **Shell** | zsh / bash                     |

Orchestra is a single static binary with no runtime dependencies.

---

## Installation

Orchestra has two release channels. Choose the one that fits your use case:

| Channel    | Who it's for           | Stability                             |
| ---------- | ---------------------- | ------------------------------------- |
| **Stable** | End users              | Production-ready, tested              |
| **Beta**   | Contributors & testers | Latest features, may have rough edges |

---

### Stable release (recommended)

For everyday users who want a reliable, production-tested binary:

```sh
curl -fsSL <raw-install-script-url> | sh
```

Replace `<raw-install-script-url>` with the raw URL for this repository's `install.sh`. The script downloads the correct pre-built binary for your Mac (`arm64` or `x86_64`), installs it to `~/.local/bin`, strips the macOS quarantine flag, and sets your release channel to **stable** so `orchestra update` tracks stable releases going forward.

---

### Beta pre-release

For contributors and testers who want the latest changes before they reach stable:

```sh
curl -fsSL <raw-install-script-url> | sh -s -- --beta
```

This downloads the latest beta pre-release and sets your release channel to **beta**. `orchestra update` will track beta releases automatically.

> **Note:** Beta builds are released on every merge to the `beta` branch. They are tagged `vX.Y.Z-beta.<build>` and marked as pre-releases on GitHub.

---

### Manual download

1. Go to this repository's releases page.
   - **Stable:** Look for the latest release without a `-beta` label.
   - **Beta:** Look for the latest pre-release tagged `vX.Y.Z-beta.*`.
2. Download the archive for your Mac:
   - Apple Silicon (M1/M2/M3/M4): `orchestra-macos-arm64.tar.gz`
   - Intel: `orchestra-macos-x86_64.tar.gz`
3. Extract and install:

```sh
tar -xzf orchestra-macos-*.tar.gz
mkdir -p ~/.local/bin
mv orchestra ~/.local/bin/orchestra
xattr -d com.apple.quarantine ~/.local/bin/orchestra 2>/dev/null || true
```

4. _(Optional)_ Write your channel preference so `orchestra update` tracks the right releases:

```sh
# For stable:
mkdir -p ~/.orchestra && echo "stable" > ~/.orchestra/channel

# For beta:
mkdir -p ~/.orchestra && echo "beta" > ~/.orchestra/channel
```

---

### Build from source

Requires [Rust](https://rustup.rs/) (stable toolchain).

```sh
git clone <repository-url>
cd <repo-name>
cargo build --release -p orchestra-cli
cp target/release/orchestra ~/.local/bin/orchestra
```

To rebuild and reinstall at any time:

```sh
cargo build --release -p orchestra-cli && cp target/release/orchestra ~/.local/bin/orchestra
```

---

### PATH setup

After installing, make sure `~/.local/bin` is on your `PATH`. Add this to your `~/.zshrc` (or `~/.bash_profile`):

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Then reload:

```sh
source ~/.zshrc
```

Verify:

```sh
orchestra --version
```

---

## Release channels

Orchestra tracks which release channel you're on and uses it to drive `orchestra update`.

| Channel  | Release trigger           | GitHub label                        |
| -------- | ------------------------- | ----------------------------------- |
| `stable` | Merge `beta → main`       | Latest release                      |
| `beta`   | Merge any branch `→ beta` | Pre-release (`vX.Y.Z-beta.<build>`) |

Your channel is stored in `~/.orchestra/channel` after installation. You can switch channels at any time with:

```sh
orchestra update --stable   # switch to stable channel
orchestra update --beta     # switch to beta channel
```

---

## Quick start

The fastest way to get going is `orchestra onboard`:

```sh
# Onboard your first codebase (interactive — detects stack, discovers files, bootstraps everything)
cd ~/Dev/myapp
orchestra onboard

# Check that everything looks healthy
orchestra doctor

# Start the background daemon so syncs happen automatically
orchestra daemon install
```

That's it. Orchestra detects your stack, backs up any existing agent files, imports them into `orchestra/controls/`, registers the codebase, renders all managed control files, and generates `orchestra/pilot.md` as the universal entry point.

For more control, you can register and sync manually:

```sh
# Register a codebase explicitly
orchestra init ~/Dev/myapp --project myapp --type backend

# Render all agent files
orchestra sync myapp

# Check sync status across all registered codebases
orchestra status
```

---

## Operational guide

For a full end-to-end walkthrough of the current system, validated command behavior, daemon lifecycle, recovery coverage, and the task/writeback model, see [guide.md](guide.md).

That guide is intentionally focused on what works well in the current source-tree version validated on March 6, 2026.

The current validated source-tree state reflected there is:

- the current generated layout is `orchestra/controls/`, `orchestra/.guide.md`, and `orchestra/pilot.md`
- the source-tree CLI command flow was validated end to end in a sandbox
- the workspace test suite passed with `cargo test --workspace -q`

### Issues and contribution ideas

If you want to contribute, start with [issues.md](issues.md). It collects the open problems and follow-up ideas found during the latest source-tree validation run.

---

## Commands

### `orchestra onboard`

**The recommended way to add a codebase.** Interactive onboarding that handles the full bootstrap workflow in one step — without losing any of your existing agent content.

```
orchestra onboard [<path>] [--project <name>] [--yes] [--force] [--migrate prompt|mechanical]
```

| Flag              | Description                                                                         |
| ----------------- | ----------------------------------------------------------------------------------- |
| `<path>`          | Path to codebase root (defaults to current directory)                               |
| `--project`, `-p` | Project group name (prompted interactively if omitted)                              |
| `--yes`, `-y`     | Accept detected project type without prompting                                      |
| `--force`         | Re-run onboarding even if already registered                                        |
| `--migrate`       | Migration mode: `prompt` (recommended) or `mechanical`; if omitted, onboarding asks |
| `--delete`        | Remove legacy agent files/folders after successful import and backup                |

**What it does:**

1. **Detects** your stack (language, framework, project type) by inspecting manifest files.
2. **Prompts** to confirm or override the detected project type.
3. **Registers** the codebase in the Orchestra registry.
4. **Scans** for existing agent files and agent libraries (`CLAUDE.md`, `.cursor/rules/`, `.github/copilot-instructions.md`, repo-level `AGENT/`, etc.).
5. **Extracts** conventions and notes from discovered files and merges them into the registry.
6. **Asks** how you want migration handled when existing files are found: prompt-assisted setup is recommended, mechanical handling is the fallback.
7. **Backs up** all existing agent files to `orchestra/backup/` with a JSON manifest.
8. **Imports** those original files into their matching generated destinations inside `orchestra/controls/`, merging conflicts into the managed files and copying extra provider assets alongside Orchestra's own generated files.
9. **Creates** `orchestra/.gitignore` to exclude backup artifacts.
10. **Runs a full sync** — renders all managed provider files into `orchestra/controls/` and writes both `orchestra/pilot.md` and `orchestra/.guide.md`.
11. **Presents migration guidance** — either a one-shot agent prompt or a mechanical summary.
12. **Optionally deletes** legacy agent files/folders if `--delete` was provided.

> By default, your existing files are preserved in-place during onboarding. Orchestra backs them up, imports their content into the matching generated locations under `orchestra/controls/`, and keeps generic legacy context in `orchestra/.guide.md` so you do not lose rules, subagents, skills, or custom instructions. Pass `--delete` if you want those legacy files/folders removed after a successful import.

**Migration modes:**

- **`--migrate prompt`** _(recommended)_ — After onboarding, Orchestra prints a one-shot setup prompt that you paste into your agent chat. The agent reads the official docs for each provider, reviews `orchestra/controls/` and `orchestra/.guide.md`, reconciles imported content there, and ensures `orchestra/pilot.md` remains the master orchestrator.

- **`--migrate mechanical`** — Orchestra imports your existing files into the matching generated locations under `orchestra/controls/`, merges discovered conventions and notes into the registry automatically, and preserves the originals unless `--delete` is set. No agent involvement is required.

```sh
# Onboard the current directory interactively (prompt mode — recommended)
orchestra onboard

# Onboard a specific path, skip prompts, use mechanical migration
orchestra onboard ~/Dev/api --project myapp --yes --migrate mechanical

# Re-run onboarding for an already-registered codebase
orchestra onboard --force
```

Example output:

```
Detected stack: TypeScript / Next.js -> frontend
Use this project type? [Y/n/change]: Y
Project name [api]: myapp
Found 3 existing agent file/folder entries.
Backed up 3 entries to /Users/you/Dev/api/orchestra/backup
Imported 3 existing file entries into /Users/you/Dev/api/orchestra/controls
Synced 'api' (12 file updates).
✓ Onboarded 'api' under project 'myapp'.
  Pilot entrypoint: /Users/you/Dev/api/orchestra/pilot.md
  Control folder: /Users/you/Dev/api/orchestra/controls

────────────────────────────────────────────────────
  🎼 ORCHESTRA SETUP PROMPT (paste this into your agent chat)
────────────────────────────────────────────────────
...
```

---

### `orchestra offboard`

**Revert onboarding and restore your codebase to its pre-Orchestra state.** Use this if something went wrong during onboarding, or if you want to remove Orchestra from a project.

```
orchestra offboard [<path>] [--project <name>] [--yes] [--recent]
```

| Flag              | Description                                                |
| ----------------- | ---------------------------------------------------------- |
| `<path>`          | Path to codebase root (defaults to current directory)      |
| `--project`, `-p` | Project group name (helps locate the codebase in registry) |
| `--yes`, `-y`     | Skip the confirmation prompt                               |
| `--recent`        | Frame the action as a recent-onboarding revert             |

**What it does:**

1. Restores all files from `orchestra/backup/` to their original locations using `manifest.json`.
2. Removes all Orchestra-managed agent files (the files rendered by `orchestra sync`).
3. Removes the project-local `orchestra/` directory.
4. Deregisters the codebase from the global registry.

Use `orchestra offboard --recent` when you want an explicit “revert recent onboarding” workflow after a bad setup run.

```sh
# Offboard the current codebase (interactive — shows what will happen first)
orchestra offboard

# Offboard a specific path, skip confirmation
orchestra offboard ~/Dev/api --yes
```

Example output:

```
Preparing to offboard 'api' under project 'myapp'.

  ✓ Backup found — pre-onboard files will be restored.
  ✓ 12 Orchestra-managed agent files will be removed.
  ✓ orchestra/ directory will be removed.
  ✓ Codebase will be deregistered from the registry.

Proceed with offboard? This cannot be undone. [y/N]: y
Restored 3 files from backup.
Removed 12 Orchestra-managed files.
Removed orchestra/ directory.
Deregistered 'api' from registry.

✓ Offboarded 'api' successfully.
  Your codebase is back to its pre-Orchestra state.
```

---

### `orchestra init`

Register a codebase in the Orchestra registry. Use this for non-interactive registration when you already know your project type.

```
orchestra init <path> --project <name> [--type <TYPE>]
```

| Flag              | Description                                                   |
| ----------------- | ------------------------------------------------------------- |
| `<path>`          | Absolute or relative path to the codebase root                |
| `--project`, `-p` | Project group name (e.g. `myapp`, `atlas`)                    |
| `--type`, `-t`    | Project category: `backend` \| `frontend` \| `mobile` \| `ml` |

**Examples:**

```sh
# Register a backend codebase under the "myapp" project
orchestra init ~/Dev/myapp/api --project myapp --type backend

# Register a frontend codebase under the same project
orchestra init ~/Dev/myapp/web --project myapp --type frontend
```

This creates a registry entry at `~/.orchestra/projects/<project>/<codebase>.yaml`.

---

### `orchestra project`

Manage codebases within the registry.

#### `orchestra project list`

List all registered codebases grouped by project:

```sh
orchestra project list
```

Example output:

```
Project: myapp
  api (/Users/you/Dev/myapp/api)
    - myapp [backend]
  web (/Users/you/Dev/myapp/web)
    - myapp [frontend]
```

#### `orchestra project add`

Add a new codebase to an existing project:

```
orchestra project add <name> [--project <project>] [--type <TYPE>]
```

| Flag              | Description                                                      |
| ----------------- | ---------------------------------------------------------------- |
| `<name>`          | Codebase name (e.g. `payments`, `dashboard`)                     |
| `--project`, `-p` | Project to add to (auto-detected if only one exists)             |
| `--type`, `-t`    | `backend` \| `frontend` \| `mobile` \| `ml` (default: `backend`) |

```sh
orchestra project add payments --project myapp --type backend
```

---

### `orchestra sync`

Render and write per-agent instruction files for one or all codebases.

```
orchestra sync <codebase>
orchestra sync --all
orchestra sync <codebase> --dry-run
```

| Flag         | Description                                           |
| ------------ | ----------------------------------------------------- |
| `<codebase>` | Name of the codebase to sync                          |
| `--all`      | Sync every registered codebase                        |
| `--dry-run`  | Show what would be written without touching any files |

Sync renders all agent-specific instruction files and skill artifacts into `orchestra/controls/`, plus the `orchestra/pilot.md` entry point. Writes are hash-gated — unchanged files are skipped for performance.

**Examples:**

```sh
# Sync a specific codebase
orchestra sync api

# Sync everything
orchestra sync --all

# Preview changes without writing
orchestra sync api --dry-run
```

Output symbols:

- `✎` — file written
- `~` — file would be written (dry-run)
- `·` — file unchanged

---

### `orchestra status`

Show staleness status across all registered codebases.

```
orchestra status
orchestra status --project <name>
orchestra status --json
```

| Flag        | Description                  |
| ----------- | ---------------------------- |
| `--project` | Filter to a specific project |
| `--json`    | Emit machine-readable JSON   |

**Status indicators:**

| Indicator         | Meaning                                      |
| ----------------- | -------------------------------------------- |
| 🟢 `CURRENT`      | Agent files are up to date                   |
| 🟡 `STALE`        | Registry changed since last sync             |
| 🔴 `MODIFIED`     | Agent file edited directly outside Orchestra |
| 🟣 `ORPHAN`       | Untracked files exist in the agent directory |
| ⚫ `NEVER SYNCED` | Codebase registered but never synced         |

```sh
# Check status of all codebases
orchestra status

# Check a specific project
orchestra status --project myapp

# Output as JSON (useful for scripts/CI)
orchestra status --json
```

---

### `orchestra diff`

Show a unified diff of what `sync` would write for a codebase — without writing anything.

```
orchestra diff <codebase>
```

```sh
orchestra diff api
```

The output is standard unified diff format and can be piped to `delta`, `diff-so-fancy`, or any diff viewer.

---

### `orchestra doctor`

Run broad health diagnostics across your Orchestra installation, registry, and managed codebases.

```
orchestra doctor [--json]
```

| Flag     | Description                                   |
| -------- | --------------------------------------------- |
| `--json` | Emit the full report as machine-readable JSON |

**Checks performed:**

| Check              | What it verifies                                          |
| ------------------ | --------------------------------------------------------- |
| Version update     | Whether a newer release is available on GitHub            |
| Binary in PATH     | Whether the running binary is on your `$PATH`             |
| Daemon socket      | Whether the Unix domain socket file exists                |
| Daemon status      | Whether the daemon process is running and responsive      |
| Registry integrity | Whether all registry YAML files load without errors       |
| Codebase paths     | Whether all registered codebase directories exist on disk |
| Pilot presence     | Whether every codebase has `orchestra/pilot.md`           |
| Staleness summary  | Count of current / stale / other codebases                |
| Managed files      | Whether all expected agent output files exist             |

```sh
# Human-readable output
orchestra doctor

# Machine-readable JSON
orchestra doctor --json
```

Example output:

```
Orchestra Doctor — v0.1.9
  ✓ version update: running latest (v0.1.9)
  ✓ binary in PATH: /Users/you/.local/bin/orchestra
  ✓ daemon socket: /Users/you/.orchestra/daemon.sock
  ✓ daemon status: running: true
  ✓ registry integrity: 3 codebase entries loaded
  ✓ codebase paths: all registered codebase paths exist
  ✓ pilot.md presence: all codebases have orchestra/pilot.md
  ✓ staleness summary: current: 3, stale: 0, other: 0
  ✓ managed files presence: all expected managed files are present
```

---

### `orchestra daemon`

Manage the Orchestra background daemon. The daemon watches your registered codebases for changes and automatically runs sync when your registry or agent files change. It also processes [writeback blocks](#writeback-protocol) when agents edit managed files.

```
orchestra daemon <SUBCOMMAND>
```

| Subcommand                         | Description                                                |
| ---------------------------------- | ---------------------------------------------------------- |
| `start`                            | Run the daemon in the foreground                           |
| `stop`                             | Gracefully stop a running daemon                           |
| `status`                           | Query the daemon's runtime status (JSON)                   |
| `install`                          | Install and activate a launchd agent (auto-start on login) |
| `uninstall`                        | Remove the launchd agent                                   |
| `logs [--lines N] [--stderr-only]` | Print recent daemon log output                             |

**Typical setup (auto-start on login):**

```sh
# Install and activate the launchd agent once
orchestra daemon install

# It will now start automatically on every login.
# To check it's running:
orchestra daemon status

# To view logs:
orchestra daemon logs
orchestra daemon logs --lines 200

# To stop and remove:
orchestra daemon uninstall
```

**Running manually (foreground):**

```sh
# Keeps running in the foreground; use Ctrl+C to stop
orchestra daemon start
```

> **Note:** The daemon uses Unix domain sockets and is macOS-only.

---

### `orchestra update`

**Auto-upgrade Orchestra to the latest release for your channel.**

`orchestra update` downloads and installs the new binary automatically — no manual download required.

```
orchestra update [--stable] [--beta]
```

| Flag       | Description                                     |
| ---------- | ----------------------------------------------- |
| _(none)_   | Check and upgrade based on your current channel |
| `--stable` | Switch to the stable channel and upgrade        |
| `--beta`   | Switch to the beta channel and upgrade          |

#### How it works

- The installed binary knows its **release channel** (`stable` or `beta`) from when it was built by CI. This is stored in `~/.orchestra/channel` after install.
- On `stable`, it checks the repository's latest stable release endpoint.
- On `beta`, it checks the most recent pre-release — the latest `vX.Y.Z-beta.*` build.
- If a newer version is found, it downloads the tarball for your architecture, extracts it, and replaces the running binary in-place.
- If already up to date, it exits cleanly.

#### Checking and upgrading

```sh
# Check and upgrade (uses your current channel)
orchestra update
```

Example output when an update is available:

```
  channel     stable
  installed   v0.1.8

  → Update available: v0.1.8 → v0.1.9

  Downloading orchestra-macos-arm64.tar.gz...

  ✓ Updated: v0.1.8 → v0.1.9
```

Example output when already up to date:

```
  channel     stable
  installed   v0.1.9

  ✓ Already on the latest stable release: v0.1.9
```

#### Switching channels

Switch from stable to beta (and upgrade immediately):

```sh
orchestra update --beta
```

Switch from beta back to stable:

```sh
orchestra update --stable
```

Your channel preference is saved to `~/.orchestra/channel` and respected on every future `orchestra update` call.

---

### `orchestra reset`

**Wipe Orchestra and all its managed files, then start fresh.** This is a full uninstall of Orchestra's configuration — binary stays intact.

```
orchestra reset --confirm [--restore-backups]
```

| Flag                | Description                                                           |
| ------------------- | --------------------------------------------------------------------- |
| `--confirm`         | Required safety flag — must be passed explicitly to prevent accidents |
| `--restore-backups` | Restore pre-onboard agent files from backups before wiping            |

**What it does:**

1. Iterates every registered codebase and removes all Orchestra-managed agent files.
2. Removes each codebase's `./orchestra/` directory.
3. Optionally restores pre-onboard backups from each `orchestra/backup/` first.
4. Wipes `~/.orchestra/` entirely (registry, hashes, daemon socket, channel).
5. Prints reinstall instructions.

```sh
# Full wipe — removes all Orchestra files and registry
orchestra reset --confirm

# Full wipe + restore pre-onboard files from backup first
orchestra reset --confirm --restore-backups
```

Example output:

```
🎼 Orchestra Reset

  Found 2 registered codebase(s):
    • myapp / api (/Users/you/Dev/myapp/api)
    • myapp / web (/Users/you/Dev/myapp/web)

  Processing 'api'...
    ✓ Removed 12 managed agent files.
    ✓ Removed orchestra/.
  Processing 'web'...
    ✓ Removed 11 managed agent files.
    ✓ Removed orchestra/.

  ✓ Removed ~/.orchestra/ (registry, hashes, channel).

✓ Orchestra has been fully reset.

  To set up a fresh installation:
    orchestra onboard

  To reinstall the latest version:
    curl -fsSL <raw-install-script-url> | sh
```

---

## Pilot entry point

Every synced codebase gets `orchestra/pilot.md` — the universal entry point that all agents read first. It also gets `orchestra/.guide.md` for durable background context. Pilot includes:

- **Quick context** — codebase name, active task count, detected stack, and links to the hidden guide and controls tree.
- **Workflow steps** — read pilot first, inspect `orchestra/controls/`, delegate when needed, implement, test, report via writeback.
- **Command reference** — the Orchestra commands available to agents.
- **Writeback protocol** — how agents report updates back to the registry.
- **Subagent delegation strategy** — guidelines for splitting work across subagents safely.
- **Worktree instructions** — rules for operating within Git worktrees.
- **Orchestra workflow** — the end-to-end sync lifecycle agents should follow.

Pilot is regenerated on every sync, so agents always see current context.

---

## Generated files

After syncing a codebase, Orchestra creates a visible `orchestra/` directory in the repo root:

```
your-codebase/
├── orchestra/
│   ├── .guide.md                   # Hidden durable repo context
│   ├── pilot.md                    # Universal agent entry point
│   ├── .gitignore                  # Excludes backup/ from version control
│   ├── backup/                     # Pre-onboard backups (if any)
│   │   └── manifest.json           # What was backed up and when
│   └── controls/
│       ├── CLAUDE.md               # Claude primary instructions
│       ├── AGENTS.md               # Codex primary instructions
│       ├── GEMINI.md               # Gemini primary instructions
│       ├── .claude/
│       │   ├── rules/orchestra.md
│       │   └── agents/
│       ├── .cursor/
│       │   ├── rules/orchestra.mdc
│       │   └── skills/orchestra-sync/skill.md
│       ├── .windsurf/
│       ├── .github/
│       ├── .codex/
│       ├── .gemini/
│       ├── .clinerules/
│       ├── .agents/
│       └── .agent/
```

Generated Orchestra files and imported user-owned agent material can coexist in the same `orchestra/controls/` tree. When a path conflicts, Orchestra keeps the managed file and preserves imported content either inline or as adjacent `*.imported.*` files.

All files are rendered from shared Tera templates with 9 common partials (header, tasks, stack, conventions, skills, orchestra workflow, subagent delegation, worktree instructions). Writes are hash-gated — unchanged files are skipped.

---

## Writeback protocol

Managed provider entrypoints expose two structured writeback surfaces. The daemon watches for these changes and applies them automatically.

The canonical task surface is the editable task block:

```md
<!-- orchestra:tasks -->

| ID    | Title                   | Status      | Description                         |
| ----- | ----------------------- | ----------- | ----------------------------------- |
| T-001 | Ship registry migration | in_progress | Coordinate rollout across providers |
| T-002 | Remove stale imports    | done        | Backfill cleanup after sync         |

<!-- /orchestra:tasks -->
```

Editing that block in any managed provider file writes the task snapshot back to the registry and re-syncs the other provider files so they all converge on the same task state.

Explicit mutations still use the update block:

```md
<!-- orchestra:update -->

task-started T-001
task-done T-002
convention-added "Always run tests before commit"
note-added "Migrated from Express to Hono"
codebase-hint /path/to/codebase

<!-- /orchestra:update -->
```

**Supported commands:**

| Command                     | Effect                                                                |
| --------------------------- | --------------------------------------------------------------------- |
| `task-started <id>`         | Mark a task as in-progress                                            |
| `task-done <id>`            | Mark a task as completed                                              |
| `task-added "<title>"`      | Create a new task                                                     |
| `convention-added "<text>"` | Add a project convention                                              |
| `note-added "<text>"`       | Add a freeform note                                                   |
| `codebase-hint <path>`      | Hint which codebase owns this file (for delegated/worktree scenarios) |

Task-block reconciliation runs before explicit update commands, so explicit commands can intentionally override the snapshot captured in the table. The update block is stripped from the file after processing. If parsing fails, an error block is written back so the agent can see what went wrong.

---

## Registry layout

Orchestra stores its registry in your home directory:

```
~/.orchestra/
├── channel                    # Release channel: "stable" or "beta"
├── daemon.sock                # Unix domain socket (when daemon is running)
├── hashes/                    # Per-codebase content hashes for staleness
└── projects/
    └── <project>/
        └── <codebase>.yaml    # Per-codebase registry file
```

Each `.yaml` file contains the codebase path, project type, detected stack, tasks, conventions, and notes. All files are human-readable and safe to inspect or commit.

**Shortcut to open directly:** To jump straight to your Orchestra registry without toggling hidden files globally, run this in your terminal:

```sh
open ~/.orchestra/projects/.
```

This opens Finder directly at the `projects/` directory so you can inspect or edit your registry YAML files.

---

## Architecture

Orchestra is built as a Rust workspace with six crates:

| Crate                | Purpose                                                                            |
| -------------------- | ---------------------------------------------------------------------------------- |
| `orchestra-cli`      | Binary entry point — all commands (`onboard`, `init`, `sync`, `doctor`, etc.)      |
| `orchestra-core`     | Registry I/O, types (`Codebase`, `ProjectName`, `ProjectType`), error types        |
| `orchestra-renderer` | Tera template engine — renders all provider files from shared + agent templates    |
| `orchestra-detector` | Stack detection (language, framework, type) and agent file scanning                |
| `orchestra-sync`     | Sync pipeline, staleness checks, hash-gated writes, backup, and writeback protocol |
| `orchestra-daemon`   | Background daemon, Unix socket protocol, launchd integration, log rotation         |

---

## Contributing

Orchestra is open source under the [MIT License](LICENSE). Contributions are welcome.

If you want a concrete starting point, begin with [issues.md](issues.md). It lists the open issues and contribution ideas discovered during the latest end-to-end validation pass.

### Branch model

| Branch | Purpose                                       |
| ------ | --------------------------------------------- |
| `main` | Production — stable releases only             |
| `beta` | Active development — all PRs merge here first |

**Never open a PR directly to `main`.** All contributions go to `beta`. Merges from `beta` → `main` happen when a release is cut.

### How to contribute

1. **Fork** the repository on GitHub.
2. **Clone** your fork:
   ```sh
   git clone <your-fork-url>
   cd <repo-name>
   ```
3. **Create a branch** off `beta`:
   ```sh
   git checkout beta
   git pull origin beta
   git checkout -b feat/my-feature
   ```
4. **Make your changes.** Run the test suite:
   ```sh
   cargo test --workspace
   ```
5. **Push** and open a pull request against the `beta` branch.

### Development tips

```sh
# Build the CLI in debug mode
cargo build -p orchestra-cli

# Run a dev binary directly
./target/debug/orchestra --help

# Rebuild and reinstall to ~/.local/bin
cargo build -p orchestra-cli && cp target/debug/orchestra ~/.local/bin/orchestra
```

### Filing issues

If you find a bug or have a feature request, open an issue on the repository host. Please include:

- Your macOS version and chip (Apple Silicon / Intel)
- The output of `orchestra --version`
- The exact command you ran and the full error output

---

## License

Orchestra is released under the [MIT License](LICENSE).
