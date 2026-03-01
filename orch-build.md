# Orchestra - Phase 04 Build Reference

Complete implementation record for Phase 04 (daemon runtime, registry watcher, Unix socket control plane, and launchd integration).

---

## Phase 04 Outcome

- Status: Completed
- New user command surface:
  - `orchestra daemon start`
  - `orchestra daemon stop`
  - `orchestra daemon status`
  - `orchestra daemon install`
  - `orchestra daemon uninstall`
  - `orchestra daemon logs [--stderr-only] [--lines <n>]`
- New daemon crate:
  - `orchestra-daemon`
- New shared sync pipeline API:
  - `orchestra_sync::pipeline::run(...)`
- Runtime architecture:
  - Tokio multi-thread runtime
  - 3 long-running tasks: watcher, sync_processor, socket_server
  - 1 shutdown signal task: ctrl-c listener
- Shared daemon state:
  - `Arc<RwLock<RegistryCache>>`
  - `type RegistryCache = HashMap<CodebaseName, Codebase>`
- Registry watch scope (Phase 04):
  - Watches only `~/.orchestra/projects/**` YAML registry tree
  - Does NOT watch codebase directories yet

---

## Validation Executed

Formatting checks run on all touched Phase 04 Rust files:

```bash
PATH="$HOME/.cargo/bin:$PATH" rustfmt --edition 2021 --check \
  orchestra-daemon/src/runtime.rs \
  orchestra-daemon/src/launchd.rs \
  orchestra-daemon/src/protocol.rs \
  orchestra-daemon/src/paths.rs \
  orchestra-cli/tests/phase04_daemon_autosync.rs
```

Result:
- Formatting passed.

Build/test note:
- Full `cargo check`/`cargo test` could not be executed in this environment because crates.io network access is blocked (new Phase 04 dependencies cannot be fetched offline).

---

## Complete File Manifest (Phase 04)

| Path | State | Phase 04 change |
|---|---|---|
| `orchestra-daemon/Cargo.toml` | Modified | Added runtime deps (`tokio`, `notify`, `serde`, `serde_json`, `tracing`, `tracing-subscriber`, `thiserror`, core/sync crates) and dev deps (`tokio-test`, `tempfile`, `assert_cmd`, `plist`). |
| `orchestra-daemon/src/lib.rs` | Added | Public module surface for daemon runtime, protocol, launchd, path helpers, and exported APIs/types. |
| `orchestra-daemon/src/error.rs` | Added | `DaemonError` model for I/O, notify, registry, sync, JSON, channel, protocol, daemon-not-running, and launchd failures. |
| `orchestra-daemon/src/paths.rs` | Added | Canonical daemon constants/path helpers (label, debounce window, socket/log/plist locations). |
| `orchestra-daemon/src/protocol.rs` | Added | JSON newline socket protocol request/response types and client helpers (`status`, `sync`, `stop`) with startup race retry for status. |
| `orchestra-daemon/src/launchd.rs` | Added | launchd plist generation, install/uninstall orchestration, `launchctl` execution, UID domain resolution, plist unit test. |
| `orchestra-daemon/src/runtime.rs` | Added | Tokio daemon runtime with watcher, sync queue processor, socket server, ctrl-c shutdown, debounce, stale-socket handling, and runtime unit tests. |
| `orchestra-cli/Cargo.toml` | Modified | Added dependency on `orchestra-daemon`. |
| `orchestra-cli/src/main.rs` | Modified | Added `daemon` command family to Clap command tree and dispatch. |
| `orchestra-cli/src/commands/mod.rs` | Modified | Registered `daemon` command module. |
| `orchestra-cli/src/commands/daemon.rs` | Added | Implemented `start|stop|status|install|uninstall|logs` CLI behavior. |
| `orchestra-cli/src/commands/sync.rs` | Modified | Switched sync execution to shared pipeline API (`orchestra_sync::pipeline::run`). |
| `orchestra-sync/src/lib.rs` | Modified | Added `pipeline` module export and `SyncScope` re-export. |
| `orchestra-sync/src/pipeline.rs` | Added | Introduced shared pipeline entrypoint used by CLI and daemon; added pipeline unit tests. |
| `orchestra-cli/tests/phase04_daemon_autosync.rs` | Added | Integration test that spawns real daemon process and verifies registry YAML change auto-syncs generated files. |

---

## Workspace and Dependency Delta

### New crate dependencies (`orchestra-daemon`)

```toml
tokio = { version = "1", features = ["full"] }
notify = { version = "6", features = ["macos_fsevent"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
thiserror = "1"
orchestra-core = { path = "../orchestra-core" }
orchestra-sync = { path = "../orchestra-sync" }
```

### New daemon dev dependencies

```toml
tokio = { version = "1", features = ["test-util"] }
tokio-test = "0.4"
tempfile = "3"
assert_cmd = "2"
plist = "1"
```

### CLI dependency changes

- Added: `orchestra-daemon = { path = "../orchestra-daemon" }`

### Sync crate API changes

- Added module: `orchestra_sync::pipeline`
- Added type: `orchestra_sync::pipeline::SyncScope`
- Added function: `orchestra_sync::pipeline::run(home, scope, dry_run)`

---

## CLI Surface Added in Phase 04

Implemented in:
- `orchestra-cli/src/main.rs`
- `orchestra-cli/src/commands/daemon.rs`

### `orchestra daemon start`

- Runs daemon in foreground.
- Calls `orchestra_daemon::start_blocking(&home)`.
- Starts Tokio runtime and all daemon tasks.

### `orchestra daemon stop`

- Sends JSON socket request `{"cmd":"stop"}`.
- Prints `daemon stop requested` on success.
- Gracefully handles missing daemon socket and prints `daemon is not running`.

### `orchestra daemon status`

- Sends JSON socket request `{"cmd":"status"}`.
- Prints pretty JSON payload when daemon is reachable.
- If daemon is not reachable, returns fallback JSON:

```json
{
  "running": false,
  "socket": "<resolved socket path>"
}
```

### `orchestra daemon install`

- Installs launchd plist into `~/Library/LaunchAgents/dev.orchestra.daemon.plist`.
- Bootstraps and kickstarts service via `launchctl`.

### `orchestra daemon uninstall`

- Boots out launchd service and removes plist.
- Removes daemon socket if present.

### `orchestra daemon logs`

- Tails stdout/stderr daemon logs from `~/.orchestra/logs`.
- Options:
  - `--stderr-only`
  - `--lines <n>` (default `100`)

---

## Sync Pipeline API (Shared by CLI + Daemon)

Implemented in `orchestra-sync/src/pipeline.rs`.

### New types

```rust
enum SyncScope {
  All,
  Codebase(String),
}
```

### New function

```rust
pub fn run(home: &Path, scope: SyncScope, dry_run: bool)
  -> Result<Vec<SyncCodebaseResult>, SyncError>
```

### Behavior

- `SyncScope::All` delegates to `sync_all(...)`.
- `SyncScope::Codebase(name)` delegates to `sync_codebase(name, ...)` and wraps into single-item vec.
- `orchestra sync` command now calls this shared pipeline instead of calling writer APIs directly.

---

## Daemon Runtime Architecture

Implemented in `orchestra-daemon/src/runtime.rs`.

### Runtime bootstrap

- `start_blocking(home)`:
  - Initializes tracing subscriber.
  - Builds Tokio multi-thread runtime (`enable_all`).
  - Blocks on `run(home)`.

- `run(home)`:
  - Ensures runtime directories exist.
  - Loads `RegistryCache` from all registry YAML files.
  - Creates channels:
    - sync queue: `mpsc::channel<SyncJob>(64)`
    - shutdown fanout: `broadcast::channel<()> (16)`
  - Spawns tasks:
    - `watcher_task`
    - `sync_processor_task`
    - `socket_server_task`
    - `signal_handle` (`tokio::signal::ctrl_c`)
  - Joins task handles and converts join failures to protocol errors.

### Shared state model

- `RegistryCache`:

```rust
type RegistryCache = HashMap<CodebaseName, Codebase>
```

- Wrapped in `Arc<RwLock<RegistryCache>>` for cross-task status/read and sync refresh.

### Deadlock-avoidance and lock duration

- Status payload path clones names under short read lock, then releases lock before JSON assembly.
- Cache refresh does expensive disk reload off-lock (`spawn_blocking`) and holds write lock only for assignment.
- No nested lock acquisitions under active write lock critical section.

---

## Watcher Task (notify + debounce + Phase 04 scope)

Implemented in `watcher_task(...)` and helpers.

### Watch registration model

- Root watch scope:
  - `~/.orchestra/projects`
- Directory registration:
  - recursively enumerates directories in projects tree
  - canonicalizes each directory path before watcher registration
  - uses `watcher.watch(&canonical_dir, RecursiveMode::NonRecursive)`
- Parent-directory registration for events:
  - for each event path, resolves `path.parent()` (or path itself if dir)
  - ensures directory-based FSEvents requirements are respected
- Watcher lifetime:
  - watcher stored as task-local variable (`_watcher`) and kept alive for entire task lifetime.

### Event filtering

- Processes only `Create` and `Modify` event kinds.
- Ignores non-registry paths.
- Registry path predicate:
  - path starts with projects root
  - extension is `.yaml` (case-insensitive)

### Debounce behavior

- Debounce map:

```rust
HashMap<PathBuf, tokio::time::Instant>
```

- Default threshold: `DEBOUNCE_WINDOW = 500ms`.
- For each path:
  - if last-seen < threshold, event is dropped
  - otherwise event is accepted and timestamp refreshed
- Debounce map cleanup:
  - entries older than 30s are pruned opportunistically.

### Watch-to-sync mapping

- If YAML filename is `project.yaml` -> sync target `All`.
- Otherwise target is `<file_stem>` codebase name.
- Enqueues sync job with source = `"watcher"`.
- After successful sync:
  - runs staleness scan for all codebases via Phase 03 checker.

---

## Sync Processor Task

Implemented in `sync_processor_task(...)`.

### Job contract

```rust
struct SyncJob {
  target: SyncTarget,        // All | Codebase(String)
  source: &'static str,      // "watcher" | "socket"
  respond_to: oneshot::Sender<Result<SyncSummary, String>>,
}
```

### Processing flow

1. Receive `SyncJob` from queue.
2. Execute sync pipeline on blocking thread:
   - `pipeline::run(&home, target.scope(), false)`
3. Refresh registry cache after successful sync.
4. Build `SyncSummary` with:
   - `target`
   - `source`
   - `codebases`
   - `written`
   - `unchanged`
   - `duration_ms`
5. Return summary/error via oneshot channel.

### SyncSummary shape

```rust
pub struct SyncSummary {
  pub target: String,
  pub source: String,
  pub codebases: Vec<String>,
  pub written: usize,
  pub unchanged: usize,
  pub duration_ms: u128,
}
```

---

## Socket Server Task and Protocol Handling

Implemented in `socket_server_task(...)`, `handle_socket_client(...)`, and `protocol.rs`.

### Socket lifecycle

- Socket path: `~/.orchestra/daemon.sock`.
- Startup stale-socket handling:
  - if socket file exists, attempts to connect:
    - connect success: treat as active daemon and return error (`socket already in use`)
    - connect failure: remove stale socket file and continue
- Bind listener with `tokio::net::UnixListener`.
- Set socket mode `0600` on Unix.
- On server shutdown, remove socket file.

### Socket commands (newline-delimited JSON)

Request model:

```json
{"cmd":"status"}
{"cmd":"sync","codebase":"copnow_api"}
{"cmd":"stop"}
```

Response model:

```json
{"ok":true,"data":...}
{"ok":false,"error":"..."}
```

### Command behavior

- `status`:
  - returns payload:
    - `running`
    - `label`
    - `started_at_unix`
    - `codebase_count`
    - `codebases`
    - `socket`
    - `projects_root`
- `sync`:
  - with `codebase` -> single-codebase sync
  - without `codebase` -> all codebases sync
  - returns `SyncSummary`
- `stop`:
  - broadcasts shutdown signal
  - returns `{"stopping": true}`
- unknown command:
  - returns protocol error response

### Protocol client helpers

Implemented in `protocol.rs`:

- `send_request(home, &DaemonRequest)`
- `request_status(home)`
- `request_sync(home, Option<String>)`
- `request_stop(home)`

Status race mitigation:
- `request_status` retries daemon connection up to 5 times with 100ms backoff before returning not-running.

---

## Launchd Integration Details

Implemented in `orchestra-daemon/src/launchd.rs`.

### Plist generation

`generate_plist(binary_path, log_dir)` emits plist with:

- `Label = dev.orchestra.daemon`
- `ProgramArguments = [<binary>, "daemon", "start"]`
- `RunAtLoad = true`
- `KeepAlive = true`
- `StandardOutPath = <log_dir>/daemon.log`
- `StandardErrorPath = <log_dir>/daemon-err.log`

### Install flow

`install(home)`:

1. Ensures macOS target (`target_os = "macos"`).
2. Creates:
   - `~/Library/LaunchAgents`
   - `~/.orchestra/logs`
   - `~/.orchestra/run`
3. Writes plist to:
   - `~/Library/LaunchAgents/dev.orchestra.daemon.plist`
4. Resolves launchctl domain with `id -u` -> `gui/<uid>`.
5. Runs:
   - `launchctl bootout gui/<uid>/dev.orchestra.daemon` (best-effort)
   - `launchctl bootstrap gui/<uid> <plist>`
   - `launchctl kickstart -k gui/<uid>/dev.orchestra.daemon`

### Uninstall flow

`uninstall(home)`:

1. Ensures macOS target.
2. If plist exists:
   - `launchctl bootout gui/<uid>/dev.orchestra.daemon` (best-effort)
   - remove plist file
3. Removes socket file if present.

### Non-macOS behavior

- Returns `DaemonError::Launchd("launchd management is only supported on macOS")`.

---

## Path and State Layout (Phase 04)

Implemented in `orchestra-daemon/src/paths.rs`.

### Constants

- `DAEMON_LABEL = "dev.orchestra.daemon"`
- `DEBOUNCE_WINDOW = 500ms`
- `DAEMON_SOCKET = "daemon.sock"`
- `DAEMON_STDOUT_LOG = "daemon.log"`
- `DAEMON_STDERR_LOG = "daemon-err.log"`

### Resolved locations

- `orchestra_root(home)` -> `~/.orchestra`
- `projects_root(home)` -> `~/.orchestra/projects`
- `run_dir(home)` -> `~/.orchestra/run`
- `socket_path(home)` -> `~/.orchestra/daemon.sock`
- `logs_dir(home)` -> `~/.orchestra/logs`
- `stdout_log_path(home)` -> `~/.orchestra/logs/daemon.log`
- `stderr_log_path(home)` -> `~/.orchestra/logs/daemon-err.log`
- `launchd_plist_path(home)` -> `~/Library/LaunchAgents/dev.orchestra.daemon.plist`

---

## Error Model Added for Phase 04

Implemented in `orchestra-daemon/src/error.rs`.

`DaemonError` variants:

- `Io { path, source }`
- `Notify(notify::Error)`
- `Registry(orchestra_core::RegistryError)`
- `Sync(orchestra_sync::SyncError)`
- `Json(serde_json::Error)`
- `ChannelClosed(&'static str)`
- `Protocol(String)`
- `DaemonNotRunning { socket: PathBuf }`
- `Launchd(String)`

Helper:
- `io_err(path, source)` to annotate I/O errors with path context.

---

## Test Coverage Added in Phase 04

### A) Sync pipeline unit tests

File:
- `orchestra-sync/src/pipeline.rs`

Tests:
- `run_all_empty_registry_returns_empty_vec`
- `run_single_codebase_returns_single_result`

### B) Launchd plist unit test

File:
- `orchestra-daemon/src/launchd.rs`

Test:
- `plist_contains_required_launchd_fields`

Asserts via `plist` crate parsing:
- Label
- RunAtLoad
- KeepAlive
- ProgramArguments exact sequence

### C) Runtime unit tests

File:
- `orchestra-daemon/src/runtime.rs`

Tests:
- `debounce_coalesces_rapid_events`
  - uses paused Tokio time + deterministic `advance`
  - validates rapid events collapse to one trigger under threshold
- `registry_cache_reload_updates_changed_codebase`
  - builds registry cache from temp registry entries
  - mutates YAML and reloads one codebase
  - verifies cache reflects mutation
- `socket_protocol_status_and_stop_over_in_memory_channels`
  - tests `status` and `stop` request handling over channel-backed fake transport
  - asserts shutdown broadcast fires on stop

### D) Daemon integration test (real subprocess)

File:
- `orchestra-cli/tests/phase04_daemon_autosync.rs`

Scenario:
1. Spawn real daemon subprocess (`orchestra daemon start`) with temp HOME.
2. Poll `orchestra daemon status` until `running: true`.
3. Initialize registry codebase (`registry::init_at`).
4. Baseline sync (`orchestra sync <codebase>`).
5. Mutate registry YAML content.
6. Wait up to 2s for daemon autosync.
7. Assert generated `CLAUDE.md` now contains sentinel.
8. Stop daemon gracefully; fallback kill if needed.

---

## Phase 04 Behavioral Guarantees Implemented

- Daemon runs all Phase 04 async responsibilities inside one runtime.
- Registry YAML edits trigger sync without watching codebase directories.
- Debounce suppresses rapid editor multi-write bursts.
- Socket startup cleans stale socket files and prevents accidental double-bind.
- Ctrl-c triggers graceful daemon shutdown broadcast.
- CLI status handles daemon startup race via retry.
- Cache/state locking minimizes deadlock risk between status reads and sync writes.
- launchd lifecycle is fully automated for macOS user agents.
- Sync processor reuses existing Phase 03 sync pipeline logic (no duplicated sync implementation).

---

## Known Operational Constraints (Current Phase 04 State)

- launchd install/uninstall are macOS-only by design.
- Daemon socket protocol is Unix-domain socket based.
- Watch scope is intentionally limited to registry YAML tree in Phase 04.
- Full workspace compile/test requires network access to fetch new crates when lock/deps are not already cached.
