---
name: orchestra-harness-alignment
description: Apply Harness Engineering principles from the strategic analysis: context systems, constraints-as-multipliers, feedback loops, and observability-first design.
---

# Objective
Translate harness principles into enforceable implementation and operational patterns.

# Use This Skill When
- Designing new agent context files, templates, or lifecycle workflows.
- Evaluating whether a solution relies on trust vs mechanical enforcement.
- Improving error messaging, remediation loops, or drift visibility.

# Procedure
1. Keep context short, structured, and pointer-first.
2. Encode conventions in templates, rules, tests, or linters.
3. Build feedback loops that let agents self-correct from observable state.
4. Add status outputs that make drift visible without manual inspection.
5. Prefer reusable constraints that scale across agents and codebases.

# Guardrails
- Avoid monolithic instruction files.
- Avoid manual synchronization checklists as primary control.
- Prefer deterministic systems over subjective reminders.

# Done Criteria
- New behavior is mechanically enforced, not advisory.
- Error outputs include direct remediation guidance.
- Cross-agent drift risk is reduced by design.
