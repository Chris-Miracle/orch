# Orchestra Gemini Style Guide

## Objectives
- Keep registry-first architecture intact.
- Favor atomic, deterministic filesystem operations.
- Preserve phase ordering from Foundation to Writeback.

## Required Engineering Behaviors
- Always state the active phase before implementing.
- Keep agent files generated from canonical registry state.
- Treat malformed writeback blocks as remediation opportunities.

## Rust Standards
- No `unwrap()`/`expect()` in library crates.
- Prefer explicit typed errors and actionable messages.
- Add tests for happy-path and error-path behavior.

## Sync Standards
- Hash-check before writes.
- Preserve no-op sync behavior.
- Never clobber non-Orchestra-owned files by default.

