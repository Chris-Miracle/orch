# Orchestra

**Manage AI coding agent files across multiple codebases.**

Orchestra is an open-source macOS CLI that keeps every AI coding agent (Claude, Copilot, Cursor, Codex, Windsurf, Gemini, and more) in sync across all your projects. It renders per-agent instruction files from a single registry, watches for changes, and propagates updates automatically — so you never manually copy-paste agent context again.

[![Release](https://img.shields.io/github/v/release/Chris-Miracle/orch)](https://github.com/Chris-Miracle/orch/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/platform-macOS-lightgrey?logo=apple)](https://github.com/Chris-Miracle/orch/releases/latest)

---

## Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
  - [Stable release (recommended)](#stable-release-recommended)
  - [Beta pre-release](#beta-pre-release)
  - [Manual download](#manual-download)
  - [Build from source](#build-from-source)
- [Release channels](#release-channels)
- [Quick start](#quick-start)
- [Commands](#commands)
  - [orchestra init](#orchestra-init)
  - [orchestra project](#orchestra-project)
  - [orchestra sync](#orchestra-sync)
  - [orchestra status](#orchestra-status)
  - [orchestra diff](#orchestra-diff)
  - [orchestra daemon](#orchestra-daemon)
  - [orchestra update](#orchestra-update)
- [Registry layout](#registry-layout)
- [Contributing](#contributing)
- [License](#license)

---

## Requirements

| | |
|---|---|
| **OS** | macOS (Apple Silicon or Intel) |
| **Shell** | zsh / bash |

Orchestra is a single static binary with no runtime dependencies.

---

## Installation

Orchestra has two release channels. Choose the one that fits your use case:

| Channel | Who it's for | Stability |
|---|---|---|
| **Stable** | End users | Production-ready, tested |
| **Beta** | Contributors & testers | Latest features, may have rough edges |

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

4. *(Optional)* Write your channel preference so `orchestra update` tracks the right releases:

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

| Channel | Release trigger | GitHub label |
|---|---|---|
| `stable` | Merge `beta → main` | Latest release |
| `beta` | Merge any branch `→ beta` | Pre-release (`vX.Y.Z-beta.<build>`) |

Your channel is stored in `~/.orchestra/channel` after installation. You can switch channels at any time with:

```sh
orchestra update --stable   # switch to stable channel
orchestra update --beta     # switch to beta channel
```

---

## Quick start

```sh
# 1. Register your first codebase
orchestra init ~/Dev/myapp --project myapp --type backend

# 2. Render agent files (CLAUDE.md, AGENTS.md, .cursor/rules/, etc.)
orchestra sync myapp

# 3. Check sync status across all registered codebases
orchestra status

# 4. Start the background daemon so syncs happen automatically on file changes
orchestra daemon start
```

---

## Commands

### `orchestra init`

Register a codebase in the Orchestra registry.

```
orchestra init <path> --project <name> [--type <TYPE>]
```

| Flag | Description |
|---|---|
| `<path>` | Absolute or relative path to the codebase root |
| `--project`, `-p` | Project group name (e.g. `myapp`, `copnow`) |
| `--type`, `-t` | Project category: `backend` \| `frontend` \| `mobile` \| `ml` |

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

| Flag | Description |
|---|---|
| `<name>` | Codebase name (e.g. `payments`, `dashboard`) |
| `--project`, `-p` | Project to add to (auto-detected if only one exists) |
| `--type`, `-t` | `backend` \| `frontend` \| `mobile` \| `ml` (default: `backend`) |

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

| Flag | Description |
|---|---|
| `<codebase>` | Name of the codebase to sync |
| `--all` | Sync every registered codebase |
| `--dry-run` | Show what would be written without touching any files |

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

| Flag | Description |
|---|---|
| `--project` | Filter to a specific project |
| `--json` | Emit machine-readable JSON |

**Status indicators:**

| Indicator | Meaning |
|---|---|
| 🟢 `CURRENT` | Agent files are up to date |
| 🟡 `STALE` | Registry changed since last sync |
| 🔴 `MODIFIED` | Agent file edited directly outside Orchestra |
| 🟣 `ORPHAN` | Untracked files exist in the agent directory |
| ⚫ `NEVER SYNCED` | Codebase registered but never synced |

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

### `orchestra daemon`

Manage the Orchestra background daemon. The daemon watches your registered codebases for changes and automatically runs sync when your registry or agent files change.

```
orchestra daemon <SUBCOMMAND>
```

| Subcommand | Description |
|---|---|
| `start` | Run the daemon in the foreground |
| `stop` | Gracefully stop a running daemon |
| `status` | Query the daemon's runtime status (JSON) |
| `install` | Install and activate a launchd agent (auto-start on login) |
| `uninstall` | Remove the launchd agent |
| `logs [--lines N] [--stderr-only]` | Print recent daemon log output |

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

| Flag | Description |
|---|---|
| *(none)* | Check and upgrade based on your current channel |
| `--stable` | Switch to the stable channel and upgrade |
| `--beta` | Switch to the beta channel and upgrade |

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
  installed   v0.1.7

  → Update available: v0.1.7 → v0.1.8

  Downloading orchestra-macos-arm64.tar.gz...

  ✓ Updated: v0.1.7 → v0.1.8
```

Example output when already up to date:
```
  channel     stable
  installed   v0.1.8

  ✓ Already on the latest stable release: v0.1.8
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


## Registry layout

Orchestra stores its registry in your home directory:

```
~/.orchestra/
└── projects/
    └── <project>/
        └── <codebase>.yaml    # Per-codebase registry file
```

Each `.yaml` file contains the codebase path, project type, and any task metadata Orchestra tracks. All files are human-readable and safe to inspect or commit.

---

## Contributing

Orchestra is open source under the [MIT License](LICENSE). Contributions are welcome.

### Branch model

| Branch | Purpose |
|---|---|
| `main` | Production — stable releases only |
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
