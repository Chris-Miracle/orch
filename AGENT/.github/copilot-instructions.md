# GitHub Copilot Instructions for Orchestra

- Treat `AGENTS.md` as canonical routing for all phase work.
- Implement Orchestra in strict phase order (01 to 05).
- Preserve deterministic outputs and atomic write semantics.
- Never delete or overwrite non-Orchestra-managed files by default.
- Prefer mechanical enforcement via templates, rules, tests, and typed schemas.
- For writeback parsing, use simple delimiter parsing and explicit error guidance.
- Include build/lint/test validation and call out remaining risks.

