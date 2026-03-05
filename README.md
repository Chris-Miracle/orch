# Orchestra

**The agent abstraction layer for AI coding tools.**

Orchestra is an open-source macOS CLI that manages every AI coding agent — Claude, Copilot, Cursor, Codex, Windsurf, Gemini, Cline, and Antigravity — across all your projects from a single source of truth. It detects your stack, discovers existing agent files, backs them up safely, then renders official-spec instruction files, subagent configs, and skill artifacts for every provider. One registry, one sync, every agent stays current.

[![Release](https://img.shields.io/github/v/release/Chris-Miracle/orch)](https://github.com/Chris-Miracle/orch/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/platform-macOS-lightgrey?logo=apple)](https://github.com/Chris-Miracle/orch/releases/latest)

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
- [Commands](#commands)
  - [orchestra onboard](#orchestra-onboard)
  - [orchestra init](#orchestra-init)
  - [orchestra project](#orchestra-project)
  - [orchestra sync](#orchestra-sync)
  - [orchestra status](#orchestra-status)
  - [orchestra diff](#orchestra-diff)
  - [orchestra doctor](#orchestra-doctor)
  - [orchestra daemon](#orchestra-daemon)
  - [orchestra update](#orchestra-update)
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
2. **Discovers** existing agent files (CLAUDE.md, .cursor/rules/, .github/copilot-instructions.md, etc.) and extracts conventions and notes from them.
3. **Backs up** those files to `.orchestra/backup/` with a JSON manifest before touching anything.
4. **Renders** official-spec instruction files, subagent definitions, and skill artifacts for every detected provider — all from your single registry YAML.
5. **Generates** `.orchestra/pilot.md` as the universal entry point that every agent reads first.
6. **Watches** for changes via a background daemon and auto-syncs when your registry or templates change.
7. **Writes back** — agents can report task updates, convention additions, and status changes directly inside managed files; Orchestra parses and applies them to the registry automatically.

---

## Supported agents

Orchestra generates provider-specific files in the format each agent expects:

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

Every provider also gets the universal entry point at `.orchestra/pilot.md`.

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
curl -fsSL https://raw.githubusercontent.com/Chris-Miracle/orch/main/install.sh | sh
```

This downloads the correct pre-built binary for your Mac (`arm64` or `x86_64`), installs it to `~/.local/bin`, strips the macOS quarantine flag, and sets your release channel to **stable** so `orchestra update` tracks stable releases going forward.

---

### Beta pre-release

For contributors and testers who want the latest changes before they reach stable:

```sh
curl -fsSL https://raw.githubusercontent.com/Chris-Miracle/orch/main/install.sh | sh -s -- --beta
```

This downloads the latest beta pre-release and sets your release channel to **beta**. `orchestra update` will track beta releases automatically.

> **Note:** Beta builds are released on every merge to the `beta` branch. They are tagged `vX.Y.Z-beta.<build>` and marked as pre-releases on GitHub.

---

### Manual download

1. Go to the [releases page](https://github.com/Chris-Miracle/orch/releases).
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
git clone https://github.com/Chris-Miracle/orch.git
cd orch
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

That's it. Orchestra detects your stack, backs up any existing agent files, registers the codebase, renders all provider files, and generates `.orchestra/pilot.md` as the universal entry point.

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

## Commands

### `orchestra onboard`

**The recommended way to add a codebase.** Interactive onboarding that handles the full bootstrap workflow in one step.

```
orchestra onboard [<path>] [--project <name>] [--yes] [--force]
```

| Flag              | Description                                            |
| ----------------- | ------------------------------------------------------ |
| `<path>`          | Path to codebase root (defaults to current directory)  |
| `--project`, `-p` | Project group name (prompted interactively if omitted) |
| `--yes`, `-y`     | Accept detected project type without prompting         |
| `--force`         | Re-run onboarding even if already registered           |

**What it does:**

1. **Detects** your stack (language, framework, project type) by inspecting manifest files.
2. **Prompts** to confirm or override the detected project type.
3. **Registers** the codebase in the Orchestra registry.
4. **Scans** for existing agent files (`CLAUDE.md`, `.cursor/rules/`, `.github/copilot-instructions.md`, etc.).
5. **Extracts** conventions and notes from discovered files and merges them into the registry.
6. **Backs up** all existing agent files to `.orchestra/backup/` with a JSON manifest.
7. **Removes** the originals (protected — never deletes Orchestra's own managed files).
8. **Creates** `.orchestra/.gitignore` to exclude backup artifacts.
9. **Runs a full sync** — renders all provider instruction files, skill artifacts, and `pilot.md`.

```sh
# Onboard the current directory interactively
orchestra onboard

# Onboard a specific path, skip prompts
orchestra onboard ~/Dev/api --project myapp --yes

# Re-run onboarding for an already-registered codebase
orchestra onboard --force
```

Example output:

```
Detected stack: TypeScript / Next.js -> frontend
Use this project type? [Y/n/change]: Y
Project name [api]: myapp
Found 3 existing agent file/folder entries.
Backed up 3 entries to /Users/you/Dev/api/.orchestra/backup
Synced 'api' (12 file updates).
✓ Onboarded 'api' under project 'myapp'.
  Pilot entrypoint: /Users/you/Dev/api/.orchestra/pilot.md
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
| `--project`, `-p` | Project group name (e.g. `myapp`, `copnow`)                   |
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

Sync renders all agent-specific instruction files, skill artifacts, and the `.orchestra/pilot.md` entry point. Writes are hash-gated — unchanged files are skipped for performance.

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
| Pilot presence     | Whether every codebase has `.orchestra/pilot.md`          |
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
  ✓ pilot.md presence: all codebases have .orchestra/pilot.md
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
- On `stable`, it checks [`/releases/latest`](https://github.com/Chris-Miracle/orch/releases/latest) — always a non-pre-release.
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

## Pilot entry point

Every synced codebase gets `.orchestra/pilot.md` — the universal entry point that all agents read first. Pilot includes:

- **Quick context** — codebase name, root path, active task count, and detected stack.
- **Workflow steps** — read tasks, implement, test, report via writeback.
- **Command reference** — the Orchestra commands available to agents.
- **Writeback protocol** — how agents report updates back to the registry.
- **Subagent delegation strategy** — guidelines for splitting work across subagents safely.
- **Worktree instructions** — rules for operating within Git worktrees.
- **Orchestra workflow** — the end-to-end sync lifecycle agents should follow.

Pilot is regenerated on every sync, so agents always see current context.

---

## Generated files

After syncing a codebase, Orchestra creates a `.orchestra/` directory alongside the provider-specific files:

```
your-codebase/
├── .orchestra/
│   ├── pilot.md              # Universal agent entry point
│   ├── .gitignore            # Excludes backup/ from version control
│   └── backup/               # Pre-onboard backups (if any)
│       └── manifest.json     # What was backed up and when
├── CLAUDE.md                 # Claude primary instructions
├── .claude/
│   ├── rules/orchestra.md    # Claude rules file
│   └── agents/
│       ├── orchestra-worker.md    # Claude subagent: worker
│       └── orchestra-reviewer.md  # Claude subagent: reviewer
├── .cursor/
│   ├── rules/orchestra.mdc       # Cursor rules
│   └── skills/orchestra-sync/skill.md
├── .windsurf/
│   ├── rules/orchestra.md
│   └── skills/orchestra-sync/skill.md
├── .github/
│   ├── copilot-instructions.md
│   └── instructions/orchestra.instructions.md
├── AGENTS.md                 # Codex primary instructions
├── .codex/skills/orchestra-sync/skill.md
├── GEMINI.md                 # Gemini primary instructions
├── .gemini/
│   ├── settings.json
│   ├── styleguide.md
│   └── skills/orchestra-sync/skill.md
├── .clinerules/orchestra.md
├── .agents/skills/orchestra-sync/skill.md  # Cline skill
├── .agent/
│   ├── rules/orchestra.md                  # Antigravity rules
│   └── skills/orchestra-sync/skill.md
```

All files are rendered from shared Tera templates with 9 common partials (header, tasks, stack, conventions, skills, orchestra workflow, subagent delegation, worktree instructions). Writes are hash-gated — unchanged files are skipped.

---

## Writeback protocol

Agents can report updates back to the Orchestra registry by embedding update blocks inside any managed file. The daemon watches for these changes and applies them automatically.

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

The block is stripped from the file after processing. If parsing fails, an error block is written back so the agent can see what went wrong.

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
   git clone https://github.com/<your-username>/orch.git
   cd orch
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

If you find a bug or have a feature request, [open an issue](https://github.com/Chris-Miracle/orch/issues). Please include:

- Your macOS version and chip (Apple Silicon / Intel)
- The output of `orchestra --version`
- The exact command you ran and the full error output

---

## License

Orchestra is released under the [MIT License](LICENSE).
