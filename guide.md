# Orchestra Guide

## Purpose

This guide documents a real end-to-end validation run against the current source tree in this repository.
It is based on two things:

- a local build of the current CLI copied into a sandbox
- a full workspace test pass from the current source tree

The validation date was March 6, 2026.

---

## Validation setup

The live command run used a copied local binary from this repo, not an older globally installed release and not `cargo run`.

Validation binary source:

```bash
target/debug/orchestra
```

Observed version:

```bash
orchestra 0.1.11
```

To avoid mutating the real machine state, everything ran inside an isolated sandbox:

- sandboxed `HOME`
- sandboxed workspace root
- copied CLI binary on sandbox `PATH`
- repo copy used as the backend onboarding target

Anonymized sample codebases used in the run:

- `sample_backend` — copy of this repository used for onboarding validation
- `sample_frontend` — minimal frontend directory registered with `init`
- `sample_docs` — extra codebase added with `project add`

Project group used in the run:

- `sample-suite`

This guide intentionally avoids personal names, local usernames, and real project labels.

---

## Command coverage

### Help surface verified

These help entrypoints were executed successfully:

```bash
orchestra --help
orchestra init --help
orchestra onboard --help
orchestra offboard --help
orchestra project --help
orchestra project list --help
orchestra project add --help
orchestra sync --help
orchestra status --help
orchestra diff --help
orchestra doctor --help
orchestra daemon --help
orchestra daemon start --help
orchestra daemon stop --help
orchestra daemon status --help
orchestra daemon install --help
orchestra daemon uninstall --help
orchestra daemon logs --help
orchestra update --help
orchestra reset --help
```

### Live execution verified

These commands were executed end to end in the sandbox:

```bash
orchestra --version
orchestra onboard <sandbox>/sample_backend --project sample-suite --yes --migrate mechanical
orchestra init <sandbox>/sample_frontend --project sample-suite --type frontend
orchestra project add sample_docs --project sample-suite --type frontend
orchestra project list
orchestra status --json
orchestra doctor --json
orchestra sync sample_backend --dry-run
orchestra diff sample_backend
orchestra sync sample_backend
orchestra sync --all
orchestra status --json
orchestra daemon start
orchestra daemon status
orchestra daemon logs --lines 20
orchestra daemon stop
orchestra update --stable
orchestra offboard <sandbox>/sample_backend --yes --recent
orchestra reset --confirm --restore-backups
orchestra status --json
cargo test --workspace -q
```

---

## Live results

## 1. Version and top-level command surface

Command:

```bash
orchestra --version
```

Observed:

- `orchestra 0.1.11`

Top-level commands present in help:

- `init`
- `project`
- `sync`
- `onboard`
- `offboard`
- `status`
- `diff`
- `daemon`
- `doctor`
- `update`
- `reset`

## 2. Onboard on a repo copy

Command:

```bash
orchestra onboard <sandbox>/sample_backend --project sample-suite --yes --migrate mechanical
```

Observed output:

- detected stack: `Rust -> backend`
- found `1` existing agent file or folder entry
- backed up `1` file entry
- synced `sample_backend` with `22 file updates`
- imported `149` existing file entries into `orchestra/controls`
- generated `orchestra/pilot.md`
- generated `orchestra/.guide.md`

Important layout observed from the current source build:

```text
orchestra/
  .guide.md
  pilot.md
  controls/
```

This confirms the current codebase is operating on the newer `controls + .guide.md` model.

## 3. Init and project add

Commands:

```bash
orchestra init <sandbox>/sample_frontend --project sample-suite --type frontend
orchestra project add sample_docs --project sample-suite --type frontend
```

Observed output:

- `sample_frontend` registered successfully
- `sample_docs` added successfully

Observed behavior detail:

- `init` stored an absolute path for `sample_frontend`
- `project add` stored `sample_docs` as a relative path
- because the directory existed in the sandbox working directory, later `doctor` and `sync --all` succeeded for it

## 4. Project list

Command:

```bash
orchestra project list
```

Observed grouping:

- `sample_backend`
- `sample_docs`
- `sample_frontend`

All three appeared under the same project group:

- `sample-suite`

## 5. Status immediately after onboard/init/add

Command:

```bash
orchestra status --json
```

Observed output:

- `sample_backend` was tracked immediately after onboarding
- `sample_docs` and `sample_frontend` were registered and visible before their first sync

Interpretation:

- onboarding, registration, and status reporting all worked in one pass
- newly registered codebases were visible immediately in the registry state

## 6. Doctor before follow-up sync

Command:

```bash
orchestra doctor --json
```

Observed checks:

- `version update`: warn
- `binary in PATH`: pass
- `daemon socket`: warn
- `daemon status`: warn
- `registry integrity`: pass
- `codebase paths`: pass
- `pilot.md presence`: warn for unsynced codebases
- `guide presence`: warn for unsynced codebases
- `staleness summary`: pass with `other: 3`
- `managed files presence`: warn for unsynced codebases

Interpretation:

- `doctor` reflects the registry and filesystem state accurately
- never-synced codebases are surfaced immediately
- the current codebase expects both `orchestra/pilot.md` and `orchestra/.guide.md`

## 7. Dry-run sync preview

Command:

```bash
orchestra sync sample_backend --dry-run
```

Observed output:

- `7 written, 15 unchanged`

Files predicted to change included:

- `orchestra/controls/CLAUDE.md`
- `orchestra/controls/.github/copilot-instructions.md`
- `orchestra/controls/AGENTS.md`
- `orchestra/controls/GEMINI.md`
- `orchestra/controls/.gemini/styleguide.md`
- `orchestra/controls/.agent/rules/orchestra.md`
- `orchestra/.guide.md`

Interpretation:

- `sync --dry-run` previewed the exact changes that a real sync would apply
- the dry-run surface matched the later real sync result

## 8. Diff after manual drift

A manual sentinel line was appended to:

- `orchestra/controls/CLAUDE.md`

Command:

```bash
orchestra diff sample_backend
```

Observed output:

- the diff included the manual sentinel line

Interpretation:

- `diff` detected real manual drift correctly
- `diff` matched the same rewrite set shown by dry-run sync

## 9. Real sync after drift

Command:

```bash
orchestra sync sample_backend
```

Observed output:

- `7 written, 15 unchanged`
- the sentinel was removed
- post-check result: `DRIFT_REMAINS=no`

Interpretation:

- the current source build repairs modified managed files correctly
- the earlier drift mismatch seen in older live-release validation is not reproduced here

## 10. Sync all and clean status

Commands:

```bash
orchestra sync --all
orchestra status --json
```

Observed output:

- `sample_backend`: `0 written, 22 unchanged`
- `sample_docs`: `22 written, 0 unchanged`
- `sample_frontend`: `22 written, 0 unchanged`

Observed final status:

- all three codebases reported `current`
- summary reported `projects: 1`, `codebases: 3`, `stale: 0`

Interpretation:

- registry-wide sync works on both already-synced and never-synced codebases
- the current generated surface is `22` tracked outputs per codebase in this run

## 11. Daemon start, status, logs, and stop

Commands:

```bash
orchestra daemon start
orchestra daemon status
orchestra daemon logs --lines 20
orchestra daemon stop
```

Observed results:

- daemon started successfully in the sandbox
- `daemon status` reported `running: true`
- socket path was created under sandbox `~/.orchestra/daemon.sock`
- status listed all three codebases
- `daemon logs` reported both log files missing
- `daemon stop` succeeded

Observed daemon stdout note:

- watcher registered `66` managed agent output paths

Interpretation:

- foreground daemon lifecycle works
- status payload is structured and useful
- `daemon logs` executed as part of the same lifecycle validation

## 12. Update command execution

Command:

```bash
orchestra update --stable
```

This was executed on a throwaway copied binary, not on the main validation binary.

Observed output:

- channel switched to `stable`
- download completed successfully
- copied binary was replaced successfully on the selected channel

Interpretation:

- the updater path executed successfully against a throwaway copied binary
- the self-replacement flow worked without touching the main validation binary

## 13. Offboard

Command:

```bash
orchestra offboard <sandbox>/sample_backend --yes --recent
```

Observed output:

- backup found
- `20` managed agent files scheduled for removal
- restored `1` file from backup
- removed `20` managed files
- removed `orchestra/` directory
- deregistered `sample_backend`

Interpretation:

- offboard works end to end on the current source build
- the pre-onboard asset that was backed up was restored successfully

## 14. Reset

Commands:

```bash
orchestra reset --confirm --restore-backups
orchestra status --json
```

Observed output:

- found `2` remaining registered codebases
- removed managed files for both
- removed their local `orchestra/` directories
- removed sandbox `~/.orchestra/`
- final status showed `0` projects and `0` codebases

Interpretation:

- reset works end to end when isolated to a sandbox HOME

---

## Automated validation

Command:

```bash
cargo test --workspace -q
```

Observed result:

- exit code `0`
- all test binaries completed without failures

This validates the current source tree in addition to the manual command run above.

---

## What to trust right now

If your goal is to understand the code under active development in this repository:

- trust this source-tree validation run
- trust the current generated layout: `orchestra/controls/`, `orchestra/.guide.md`, and `orchestra/pilot.md`
- trust the passing workspace test suite

If your goal is to understand stable-release behavior:

- re-run this same process against the current published binary
- do not assume the latest release matches the local source build version

For open issues and contribution starting points discovered during this validation run, see [issues.md](/Users/chris/Dev/OS/orch/issues.md).
