---
name: orchestra-registry-foundation
description: Design and maintain the Phase 01 Orchestra YAML registry, Rust core types, atomic save/load, and CLI bootstrap commands.
---

# Objective
Build or modify the Orchestra registry as the single source of truth.

# Use This Skill When
- Creating or changing `Codebase`, `Task`, `Skill`, `Subagent`, or `AgentConfig` schema.
- Implementing `orchestra init`, project listing, or registry validation.
- Debugging YAML parse/save failures, file permissions, or atomic writes.

# Procedure
1. Update core domain types first, then serialization, then CLI wiring.
2. Keep all path handling with `PathBuf` and return typed errors.
3. Use atomic write in target directory (`.tmp` then `rename`).
4. Validate roundtrip serde for all changed structs.
5. Verify Unix permission targets (dirs `0700`, YAML `0600`).

# Guardrails
- Never use `unwrap()`/`expect()` in library crates.
- Keep registry deterministic and human-readable.
- Emit actionable parse errors with file path and line context.

# Done Criteria
- `cargo build --workspace` passes.
- `cargo clippy --workspace -- -D warnings` passes.
- Roundtrip and error-path tests pass for registry flows.
