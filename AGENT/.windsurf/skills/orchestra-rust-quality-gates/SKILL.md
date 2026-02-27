---
name: orchestra-rust-quality-gates
description: Enforce non-functional engineering requirements: clippy cleanliness, test rigor, performance limits, safety defaults, and deterministic behavior.
---

# Objective
Keep Orchestra production-grade under strict quality, safety, and performance constraints.

# Use This Skill When
- Defining CI checks or local quality gates.
- Reviewing changes for safety, determinism, and platform correctness.
- Hardening errors, docs, and test coverage around critical code paths.

# Procedure
1. Run build, clippy, and test suites before merge.
2. Add/expand tests for every new error path and edge case.
3. Verify atomic write semantics and no destructive file operations.
4. Confirm command behavior under `--dry-run` where applicable.
5. Re-check startup, memory, and sync latency budgets.

# Guardrails
- No silent failures.
- No unsafe overwrite/delete behavior by default.
- Preserve offline-first and zero-API-key assumptions.

# Done Criteria
- Quality gate commands pass cleanly.
- Added functionality includes failure-mode tests.
- Runtime and safety targets remain within documented bounds.
