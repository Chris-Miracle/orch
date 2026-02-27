---
applyTo: "**/*.rs,**/Cargo.toml,**/Cargo.lock"
---

Prioritize correctness and determinism over speed.

- Use typed errors and `?` propagation.
- Keep path handling with `PathBuf`.
- Maintain atomic write semantics and no-op hash behavior.
- Add focused tests for new failure modes.

