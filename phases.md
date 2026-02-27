# Orchestra — Phase Reference

---

## Getting Started (First Time)

Orchestra is a native CLI tool written in Rust. No pip, no npm — you install it once at the OS level via Cargo.

**Prerequisite: Rust toolchain**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Clone and install**
```bash
git clone https://github.com/yourorg/orch.git
cd orch
cargo install --path orchestra-cli
```

This puts the `orchestra` binary in `~/.cargo/bin/` (already on your PATH after rustup setup). You can now run `orchestra` from anywhere.

**Verify**
```bash
orchestra --version
orchestra --help
```

---

## Phase 01 — Foundation ✅

*Registry core, CLI skeleton, stack detector.*

### What was implemented

| Area | Detail |
|---|---|
| Registry | YAML file at `~/.orchestra/registry.yaml` (created on first `init`) |
| CLI | `orchestra init`, `orchestra project list`, `orchestra project add` |
| Detector | Reads indicator files to auto-detect language + framework |

### Using it

**Register a codebase from its directory:**
```bash
cd /path/to/your/project
orchestra init . --project myapp --type backend
```

**Or specify the path directly:**
```bash
orchestra init ~/code/myapi --project myapi --type backend
orchestra init ~/code/mobile --project app --type mobile
```

Supported types: `backend` · `frontend` · `mobile` · `ml`

**List everything registered:**
```bash
orchestra project list
```

**Add another project to the first registered codebase:**
```bash
orchestra project add payments --type backend
```

**Where your data lives:**
```
~/.orchestra/registry.yaml   ← single source of truth
```

### Development (without installing)

```bash
# Run without installing
cargo run --bin orchestra -- init . --project myapp --type backend
cargo run --bin orchestra -- project list

# Tests (77 passing)
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
```

---

## Phase 02 — Template Engine 🔜

*Per-agent file rendering, hash store, atomic writes.*
Coming: `orchestra sync` — renders agent config files from registry state.

---

## Phase 03 — Staleness / Observability 🔜

*Status signals, diff output, stale-file detection.*

---

## Phase 04 — Daemon / Watcher 🔜

*Background autosync, launchd integration, file watching.*

---

## Phase 05 — Writeback Protocol 🔜

*Agents write back task completions; Orchestra propagates them.*
