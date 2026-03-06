# Orchestra Issues And Contribution Ideas

This document tracks the concrete issues and follow-up ideas discovered during the March 6, 2026 source-tree validation run.

If you want to contribute, this is a good place to start.

---

## High priority

### Onboard does not finish clean after legacy import

Observed during sandbox validation:

- `orchestra onboard ... --migrate mechanical` completed successfully
- immediate `orchestra status --json` reported the onboarded repo copy as `modified`
- a follow-up `orchestra sync` was still required to converge generated files

Why it matters:

- onboarding should ideally leave a freshly managed codebase in `current`
- requiring an immediate second sync is surprising and weakens trust in the bootstrap flow

Good contribution target:

- make onboarding end in a clean synced state after import
- add an end-to-end CLI test that asserts `current` immediately after onboarding

---

## Medium priority

### `update --stable` can downgrade a source build

Observed during validation:

- local build reported `v0.1.11`
- stable channel target resolved to `v0.1.10`
- the updater successfully replaced the throwaway copied binary with the older stable release

Why it matters:

- the updater is working as implemented, but the behavior is easy to misread if the local build version is ahead of the latest published stable tag

Good contribution target:

- improve update messaging when the selected release channel points to an older published version than the running binary
- decide whether downgrade confirmation or a clearer warning is appropriate

### `daemon logs` may report missing log files after a successful foreground lifecycle

Observed during validation:

- `orchestra daemon start`, `status`, and `stop` worked
- `orchestra daemon logs --lines 20` reported missing log files in the same sandbox run

Why it matters:

- foreground daemon behavior is usable, but logs are less reliable than the rest of the lifecycle

Good contribution target:

- make foreground startup create expected log targets consistently, or document a different log path/runtime behavior

---

## Lower priority

### `project add` path behavior is easy to misunderstand

Observed during validation:

- `init` stored an absolute path for the registered codebase
- `project add sample_docs ...` stored `sample_docs` as a relative path
- the flow worked only because the directory existed relative to the sandbox working directory

Why it matters:

- relative-path persistence is valid but ambiguous from a user perspective

Good contribution target:

- decide whether `project add` should persist absolute paths, require an explicit path argument, or reject unresolved relative targets

### `daemon install` and `daemon uninstall` are not safely sandboxable today

Observed during validation:

- they were help-verified only, not live-run
- current implementation targets real `launchctl` state and hardcodes `/usr/local/bin/orchestra`

Why it matters:

- this makes the launchd path harder to validate safely in end-to-end automation

Good contribution target:

- make launchd install/uninstall use the actual running binary path
- make the launchd workflow easier to test without mutating real user session state
