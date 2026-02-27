# Claude Code Context for Orchestra

## Read First
- `/Users/chris/Dev/OS/orch/AGENT/AGENTS.md`
- `/Users/chris/Dev/OS/orch/AGENT/.claude/agents/`
- `/Users/chris/Dev/OS/orch/AGENT/.claude/skills/`

## Execution Pattern
1. Identify current phase (01-05).
2. Select the most relevant subagent.
3. Pull only the necessary skill(s).
4. Implement changes with deterministic behavior.
5. Validate with build/lint/tests and report residual risks.

## Non-Negotiables
- No registry drift: YAML is canonical.
- No partial writes: atomic write only.
- No unbounded context files: prefer map-like guidance with references.
- For malformed writeback, emit explicit `orchestra:error` remediation.

