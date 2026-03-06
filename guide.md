# Orchestra Guide

## Purpose

This is the live operator guide for Orchestra based on two things:

- a real end-to-end run of the installed `orchestra` binary
- a full automated test pass of the current source tree

This file corrects the earlier mistake of documenting `cargo run ...` flows instead of the installed CLI. The command coverage below was executed with the real binary:

```bash
orchestra ...
```

Not with:

```bash
cargo run -p orchestra-cli -- ...
```

---

## What was actually run

Installed binary:

```bash
orchestra --version
```

Observed:

- `orchestra 0.1.10`
- binary path: `/Users/chris/.local/bin/orchestra`

A sandboxed validation environment was used so the live run would not mutate the real local Orchestra registry.

Sandbox strategy:

- temp HOME was isolated under a temporary sandbox
- a Rust sample codebase `atlas_api` was onboarded
- a frontend sample codebase `atlas_web` was initialized manually
- a third sample codebase `atlas_docs` was added through `orchestra project add`
- destructive commands were executed only inside that sandbox

---

## Command coverage

These are the real installed-CLI commands that were executed.

### Binary and help

```bash
orchestra --version
orchestra --help
orchestra project --help
orchestra project add --help
orchestra update --help
```

### Registry bootstrap and onboarding

```bash
orchestra onboard <sandbox>/atlas_api --project atlas --yes --migrate mechanical
orchestra init <sandbox>/atlas_web --project atlas --type frontend
orchestra project add atlas_docs --project atlas --type frontend
orchestra project list
```

### Status, doctor, diff, and sync

```bash
orchestra status --json
orchestra doctor --json
orchestra sync atlas_api --dry-run
orchestra diff atlas_api
orchestra sync atlas_api
orchestra sync --all
orchestra status --json
```

### Daemon lifecycle

```bash
orchestra daemon start
orchestra daemon status
orchestra daemon logs --lines 20
orchestra daemon stop
```

### Recovery and teardown

```bash
orchestra offboard <sandbox>/atlas_api --yes --recent
orchestra reset --confirm --restore-backups
orchestra status --json
```

### Automated source-tree validation

```bash
cargo test --workspace -- --nocapture
```

### Not executed live

```bash
orchestra update
```

Reason:

- `orchestra update` self-replaces the installed binary and depends on remote release state
- running it during validation would mutate the actual local installation rather than only the sandbox registry
- it is the only CLI command intentionally not executed end to end

---

## Live installed-binary results

## 1. Onboard

Command:

```bash
orchestra onboard <sandbox>/atlas_api --project atlas --yes --migrate mechanical
```

Observed output:

- detected stack: `Rust -> backend`
- found `2` existing agent file or folder entries
- backed up `2` file entries
- synced `atlas_api` with `21 file updates`
- reported mechanical migration complete

Most important observed paths from the installed binary:

- `orchestra/pilot.md`
- `orchestra/control/`
- `orchestra/control/source/`

This is critical: the installed `0.1.10` binary did not use the newer in-repo `orchestra/controls/` plus `.guide.md` layout. It used the older live layout:

```text
orchestra/
  control/
  control/source/
  pilot.md
```

It also reported this exact behavior:

- existing files were mirrored into `orchestra/control/source`
- generated files were written into `orchestra/control`

So the installed CLI currently behaves differently from the source tree in this repository.

## 2. Init

Command:

```bash
orchestra init <sandbox>/atlas_web --project atlas --type frontend
```

Observed output:

- `✓ Registered 'atlas_web' under project 'atlas'`
- saved registry YAML under `~/.orchestra/projects/atlas/atlas_web.yaml`

## 3. Project add

Command:

```bash
orchestra project add atlas_docs --project atlas --type frontend
```

Observed output:

- `✓ Added 'atlas_docs' to project 'atlas'`

Observed behavior detail:

- `project add` produced a registry entry for `atlas_docs`
- before a real directory was created for it, `doctor` flagged it as missing
- after the directory was created, `sync --all` succeeded for it

This means `project add` can create a registry entry ahead of real filesystem setup.

## 4. Project list

Command:

```bash
orchestra project list
```

Observed output grouped the three codebases correctly under one project:

- `atlas_api`
- `atlas_docs`
- `atlas_web`

## 5. Status before full sync

Command:

```bash
orchestra status --json
```

Observed output after onboard and init:

- `atlas_api`: `current`
- `atlas_docs`: `never_synced`
- `atlas_web`: `never_synced`

Interpretation:

- onboarding performs an initial sync for the onboarded codebase
- `init` and `project add` only register codebases; they do not sync them

## 6. Doctor before full sync

Command:

```bash
orchestra doctor --json
```

Observed checks:

- `version update`: pass
- `binary in PATH`: pass
- `registry integrity`: pass
- `daemon socket`: warn
- `daemon status`: warn
- `codebase paths`: warn because `atlas_docs` did not exist yet
- `pilot.md presence`: warn for unsynced codebases
- `managed files presence`: warn for unsynced codebases

Interpretation:

- `doctor` reflects the registry state accurately
- unsynced or missing codebases appear immediately as warnings
- the installed binary still checks only `pilot.md`, not the newer `.guide.md` artifact from the source tree

## 7. Dry-run sync

Command:

```bash
orchestra sync atlas_api --dry-run
```

Observed output:

- `0 written, 21 unchanged`
- managed files listed under `orchestra/control/...`
- pilot listed at `orchestra/pilot.md`

This confirms the installed binary's live managed surface is currently `21` files, not the newer `controls + .guide.md` shape from source.

## 8. Diff after manual drift

A manual edit was introduced into the installed binary's managed file:

- `orchestra/control/CLAUDE.md`

Command:

```bash
orchestra diff atlas_api
```

Observed output:

- diff correctly showed the manual marker in `orchestra/control/CLAUDE.md`

Interpretation:

- `diff` can detect drift in managed files correctly

## 9. Real sync after drift

Command:

```bash
orchestra sync atlas_api
```

Observed output:

- `✓ 'atlas_api' synced (0 written, 21 unchanged)`

This is a real bug in the installed CLI validation run.

Why it is a bug:

- `diff` saw drift in `orchestra/control/CLAUDE.md`
- later `status` still reported the codebase as modified
- but `sync` still claimed nothing was written

So for the installed binary, the live behavior is inconsistent:

- `diff` sees the manual change
- `status` sees the manual change
- `sync` does not repair the manual change

## 10. Sync all

Command:

```bash
orchestra sync --all
```

Observed output:

- `atlas_api`: `0 written, 21 unchanged`
- `atlas_docs`: `21 written, 0 unchanged`
- `atlas_web`: `21 written, 0 unchanged`

Interpretation:

- registry-wide sync works for never-synced codebases
- the same inconsistency remained for the already-drifted `atlas_api`

## 11. Status after drift and sync

Command:

```bash
orchestra status --json
```

Observed output:

- `atlas_api`: `modified`
- detail: `orchestra/control/CLAUDE.md edited`
- `atlas_docs`: `current`
- `atlas_web`: `current`

This confirms the installed binary left `atlas_api` drift unresolved even after `sync` and `sync --all`.

## 12. Daemon lifecycle

Commands:

```bash
orchestra daemon start
orchestra daemon status
orchestra daemon logs --lines 20
orchestra daemon stop
```

Observed results:

- daemon started successfully
- `daemon status` returned `running: true`
- socket path was created inside the sandbox `~/.orchestra/daemon.sock`
- `daemon logs` reported both log files missing
- `daemon stop` worked

Observed warnings at startup:

- hash store entries did not match managed watcher paths
- warnings referenced `orchestra/pilot.md`

Interpretation:

- daemon lifecycle works
- watcher path logic in the installed binary appears inconsistent with recorded managed paths
- the warning is not just environmental noise here; it is tied to the installed path model

## 13. Offboard

Command:

```bash
orchestra offboard <sandbox>/atlas_api --yes --recent
```

Observed output:

- restored `2` files from backup
- removed `20` managed files
- removed `orchestra/` directory
- deregistered `atlas_api`

Interpretation:

- offboard works end to end in the installed binary
- it restored the pre-onboard legacy files successfully

## 14. Reset

Command:

```bash
orchestra reset --confirm --restore-backups
```

Observed output:

- found `2` remaining registered codebases
- removed managed files for both
- removed project-local `orchestra/` directories
- removed sandbox `~/.orchestra/`

Follow-up command:

```bash
orchestra status --json
```

Observed output:

- `projects: 0`
- `codebases: 0`

Interpretation:

- reset works end to end when isolated to a sandbox HOME

---

## Full automated source-tree validation

Command:

```bash
cargo test --workspace -- --nocapture
```

Observed result:

- full workspace suite passed

Important coverage areas from the current source tree:

- onboarding
- provider-aware imports
- hidden guide generation
- task-table parsing and propagation
- dry-run sync
- status and diff
- daemon autosync
- offboard
- reset
- backup restore

---

## Source tree versus installed binary

This repository currently contains a newer architecture than the installed `orchestra 0.1.10` binary that was validated live.

## Current source tree behavior

The source tree in this repo is implementing or documenting:

- `orchestra/controls/`
- `orchestra/.guide.md`
- provider-aware imports into matching generated destinations
- a canonical `<!-- orchestra:tasks -->` block
- offboard and reset flows covered by tests

## Installed binary behavior actually observed live

The installed binary currently uses:

- `orchestra/control/`
- `orchestra/control/source/`
- `orchestra/pilot.md`
- no generated `.guide.md` in the live installed flow that was validated

That means the repo is ahead of the installed binary.

This distinction matters:

- if you are documenting the current code in this repo, the newer `controls + .guide.md` model is relevant
- if you are documenting what users get from the installed `orchestra 0.1.10` binary today, the live behavior is still `control + control/source`

---

## Real issues found during live installed-CLI validation

## High severity

- `sync` did not repair a modified managed file even though both `diff` and `status` detected the drift
- installed path model does not match the current source-tree model documented elsewhere in the repo

## Medium severity

- daemon startup warns that hash store entries do not match managed watcher paths
- `project add` can leave the registry pointing at a codebase path that does not yet exist

## Low severity

- `daemon logs` reports missing log files in this foreground lifecycle path
- command help exists for `update`, but live update was intentionally not executed because it mutates the installed binary itself

---

## Source-tree follow-up after live validation

After the installed `0.1.10` validation exposed the drift mismatch, the current source tree was updated and re-tested.

What changed in source:

- `orchestra-sync` no longer treats a stored hash match as sufficient proof that a file is unchanged
- sync now re-hashes the on-disk managed file before returning `unchanged`
- legacy hash-store keys pointing at root-level outputs, `.orchestra/controls/...`, or `orchestra/control/...` are migrated in memory to the current `orchestra/controls/...` layout when a codebase is loaded

What this means:

- the current repo no longer reproduces the specific bug where `diff` and `status` detect a modified managed file but `sync` reports `0 written`
- the current repo now carries an explicit compatibility path for older managed-file layouts when reading hash stores
- the installed `orchestra 0.1.10` binary remains behind these fixes until a new release is built and installed

Validation run for the source-tree follow-up:

- targeted `orchestra-sync` regression tests passed for drift rewrite and legacy hash-key migration
- targeted CLI regression test passed for `orchestra sync` repairing a modified managed file and returning the codebase to `current`

---

## What to trust right now

If your goal is to understand the code under active development in this repository:

- trust the source tree and the passing workspace tests

If your goal is to understand the behavior a user gets from the installed `orchestra 0.1.10` binary today:

- trust the live CLI results in this guide

Those are not currently the same thing.

---

## Recommended next steps

1. Cut a new release so the installed `orchestra` binary matches the current `orchestra/controls` plus `.guide.md` source-tree model and includes the sync/hash-store fixes.
2. Re-run the same installed-CLI validation against that new release to confirm the live binary no longer reproduces the old `sync` mismatch.
3. Re-check daemon watcher alignment on the released binary now that legacy hash keys are normalized in the source tree.
4. Decide whether `project add` should create the filesystem path or reject nonexistent targets.
