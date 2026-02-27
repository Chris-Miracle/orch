---
name: orchestra-staleness-observability
description: Implement Phase 03 observability features: status signals, staleness detection, modified/orphan detection, and sync diff visibility.
---

# Objective
Provide accurate staleness and drift signals across all managed codebases.

# Use This Skill When
- Implementing `orchestra status` or `orchestra diff`.
- Adding drift signals (`Stale`, `Modified`, `Orphan`).
- Investigating mismatches between registry and rendered agent files.

# Procedure
1. Compare registry mtime with managed file mtimes.
2. Recompute hashes and compare against stored hashes.
3. Detect orphan files not present in hash tracking.
4. Surface concise status output with actionable reasons.
5. Include per-codebase active-task and last-sync visibility.

# Guardrails
- Status must match real filesystem state.
- Avoid hidden heuristics; keep logic deterministic and testable.
- Preserve fast CLI startup and low-latency status checks.

# Done Criteria
- Manual file edits are detected as `Modified`.
- Registry edits before sync are detected as `Stale`.
- Orphan managed files are consistently surfaced.
