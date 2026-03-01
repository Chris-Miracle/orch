use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio::time::Instant;

use orchestra_core::{
    registry,
    types::{Codebase, CodebaseName},
};
use orchestra_sync::{
    pipeline::{self, SyncScope},
    staleness, SyncCodebaseResult, WriteResult,
};

use crate::error::{io_err, DaemonError};
use crate::paths::{projects_root, run_dir, socket_path, DEBOUNCE_WINDOW};
use crate::protocol::{DaemonRequest, DaemonResponse};

pub type RegistryCache = HashMap<CodebaseName, Codebase>;

/// Per-codebase last-successful-sync timestamps (Unix seconds).
/// Key: codebase name string. Value: unix seconds at last successful sync.
pub type SyncTimestamps = HashMap<String, u64>;

#[derive(Debug, Clone)]
enum SyncTarget {
    All,
    Codebase(String),
}

impl SyncTarget {
    fn scope(&self) -> SyncScope {
        match self {
            SyncTarget::All => SyncScope::All,
            SyncTarget::Codebase(name) => SyncScope::Codebase(name.clone()),
        }
    }

    fn label(&self) -> String {
        match self {
            SyncTarget::All => "all".to_string(),
            SyncTarget::Codebase(name) => name.clone(),
        }
    }
}

struct SyncJob {
    target: SyncTarget,
    source: &'static str,
    respond_to: oneshot::Sender<Result<SyncSummary, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncSummary {
    pub target: String,
    pub source: String,
    pub codebases: Vec<String>,
    pub written: usize,
    pub unchanged: usize,
    pub duration_ms: u128,
}

/// Start the daemon runtime and block the current thread until it exits.
pub fn start_blocking(home: &Path) -> Result<(), DaemonError> {
    init_tracing();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| io_err("tokio-runtime", e))?;
    runtime.block_on(run(home.to_path_buf()))
}

/// Run the daemon runtime.
pub async fn run(home: PathBuf) -> Result<(), DaemonError> {
    ensure_runtime_dirs(&home)?;

    let cache = std::sync::Arc::new(RwLock::new(load_registry_cache(&home)?));
    let sync_timestamps: std::sync::Arc<RwLock<SyncTimestamps>> =
        std::sync::Arc::new(RwLock::new(HashMap::new()));
    let started_at_unix = unix_seconds_now();

    let (sync_tx, sync_rx) = mpsc::channel::<SyncJob>(64);
    let (shutdown_tx, _) = broadcast::channel::<()>(16);

    let watcher_handle = {
        let shutdown = shutdown_tx.clone();
        let home = home.clone();
        let sync_tx = sync_tx.clone();
        tokio::spawn(async move {
            let result = watcher_task(home, sync_tx, shutdown.subscribe()).await;
            let _ = shutdown.send(());
            result
        })
    };

    let processor_handle = {
        let shutdown = shutdown_tx.clone();
        let home = home.clone();
        let cache = cache.clone();
        let timestamps = sync_timestamps.clone();
        tokio::spawn(async move {
            let result =
                sync_processor_task(home, cache, timestamps, sync_rx, shutdown.subscribe()).await;
            let _ = shutdown.send(());
            result
        })
    };

    let socket_handle = {
        let shutdown = shutdown_tx.clone();
        let home = home.clone();
        let cache = cache.clone();
        let sync_tx = sync_tx.clone();
        let timestamps = sync_timestamps.clone();
        tokio::spawn(async move {
            let result = socket_server_task(
                home,
                cache,
                timestamps,
                sync_tx,
                shutdown.clone(),
                shutdown.subscribe(),
                started_at_unix,
            )
            .await;
            let _ = shutdown.send(());
            result
        })
    };

    let rotation_handle = {
        let shutdown = shutdown_tx.clone();
        let home = home.clone();
        tokio::spawn(async move {
            let result = log_rotation_task(home, shutdown.subscribe()).await;
            let _ = shutdown.send(());
            result
        })
    };

    let signal_handle = {
        let shutdown = shutdown_tx.clone();
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown.subscribe();
            tokio::select! {
                _ = shutdown_rx.recv() => Ok(()),
                signal = tokio::signal::ctrl_c() => {
                    match signal {
                        Ok(()) => {
                            tracing::info!("received ctrl-c, shutting down daemon");
                            let _ = shutdown.send(());
                            Ok(())
                        }
                        Err(err) => Err(DaemonError::Protocol(format!("ctrl-c handler failed: {err}"))),
                    }
                }
            }
        })
    };

    let (watcher_result, processor_result, socket_result, rotation_result, signal_result) =
        tokio::join!(
            watcher_handle,
            processor_handle,
            socket_handle,
            rotation_handle,
            signal_handle
        );

    handle_join("watcher", watcher_result)?;
    handle_join("sync_processor", processor_result)?;
    handle_join("socket_server", socket_result)?;
    handle_join("log_rotation", rotation_result)?;
    handle_join("signal_handler", signal_result)?;
    Ok(())
}

async fn watcher_task(
    home: PathBuf,
    sync_tx: mpsc::Sender<SyncJob>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), DaemonError> {
    let projects = projects_root(&home);
    if !projects.exists() {
        fs::create_dir_all(&projects).map_err(|e| io_err(&projects, e))?;
    }

    // Canonicalize so that FSEvents paths (which arrive as real paths, e.g.
    // /private/var/... on macOS) match the `starts_with` checks below.
    let projects = fs::canonicalize(&projects).unwrap_or(projects);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<notify::Result<Event>>();
    let mut _watcher: RecommendedWatcher = recommended_watcher(move |event| {
        let _ = event_tx.send(event);
    })?;

    let mut watched_dirs = HashSet::new();
    register_projects_tree(&mut _watcher, &mut watched_dirs, &projects)?;

    let mut debounce = HashMap::<PathBuf, Instant>::new();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            event = event_rx.recv() => {
                let Some(event) = event else { break };
                let event = match event {
                    Ok(event) => event,
                    Err(err) => {
                        tracing::warn!(error = %err, "watcher event error");
                        continue;
                    }
                };
                if !is_relevant_event_kind(&event.kind) {
                    continue;
                }

                for path in event.paths {
                    // FSEvents is directory-based; always register parent directory.
                    if let Some(watch_dir) = directory_to_watch(&path) {
                        if watch_dir.starts_with(&projects) && watch_dir.exists() {
                            register_projects_tree(&mut _watcher, &mut watched_dirs, &watch_dir)?;
                        }
                    }

                    if !is_registry_yaml(&path, &projects) {
                        continue;
                    }

                    if !should_process_event(&mut debounce, &path, Instant::now()) {
                        continue;
                    }

                    let target = sync_target_for_path(&path);

                    match enqueue_sync(&sync_tx, target, "watcher").await {
                        Ok(summary) => {
                            tracing::info!(
                                target = %summary.target,
                                written = summary.written,
                                unchanged = summary.unchanged,
                                duration_ms = summary.duration_ms,
                                "watcher-triggered sync completed",
                            );
                            if let Err(err) = run_staleness_scan(home.clone()).await {
                                tracing::warn!(error = %err, "staleness scan after sync failed");
                            }
                        }
                        Err(err) => {
                            tracing::error!(error = %err, "watcher-triggered sync failed");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn sync_processor_task(
    home: PathBuf,
    cache: std::sync::Arc<RwLock<RegistryCache>>,
    timestamps: std::sync::Arc<RwLock<SyncTimestamps>>,
    mut sync_rx: mpsc::Receiver<SyncJob>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), DaemonError> {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            maybe_job = sync_rx.recv() => {
                let Some(job) = maybe_job else { break };
                let started = Instant::now();

                let target = job.target.clone();
                let source = job.source;
                let home_for_sync = home.clone();
                let sync_result = tokio::task::spawn_blocking(move || {
                    pipeline::run(&home_for_sync, target.scope(), false)
                })
                .await
                .map_err(|err| DaemonError::Protocol(format!("sync task join error: {err}")))?;

                let outcome = match sync_result {
                    Ok(results) => {
                        let refreshed = refresh_cache(home.clone(), cache.clone()).await;
                        match refreshed {
                            Ok(()) => {
                                // Record successful sync timestamp for each affected codebase.
                                let now = unix_seconds_now();
                                let mut ts = timestamps.write().await;
                                for name in results.iter().map(|r| r.codebase_name.as_str()) {
                                    ts.insert(name.to_string(), now);
                                }
                                // Drop write lock before building summary.
                                drop(ts);
                                Ok(build_sync_summary(job.target, source, results, started.elapsed()))
                            }
                            Err(err) => Err(err.to_string()),
                        }
                    }
                    Err(err) => Err(err.to_string()),
                };

                let _ = job.respond_to.send(outcome);
            }
        }
    }

    Ok(())
}

async fn socket_server_task(
    home: PathBuf,
    cache: std::sync::Arc<RwLock<RegistryCache>>,
    timestamps: std::sync::Arc<RwLock<SyncTimestamps>>,
    sync_tx: mpsc::Sender<SyncJob>,
    shutdown_tx: broadcast::Sender<()>,
    mut shutdown_rx: broadcast::Receiver<()>,
    started_at_unix: u64,
) -> Result<(), DaemonError> {
    let run = run_dir(&home);
    if !run.exists() {
        fs::create_dir_all(&run).map_err(|e| io_err(&run, e))?;
    }

    let socket = socket_path(&home);
    prepare_socket_for_bind(&socket)?;

    let listener = UnixListener::bind(&socket).map_err(|e| io_err(&socket, e))?;
    set_socket_permissions(&socket)?;

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|e| io_err(&socket, e))?;
                let home = home.clone();
                let cache = cache.clone();
                let timestamps = timestamps.clone();
                let sync_tx = sync_tx.clone();
                let shutdown_tx = shutdown_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_socket_client(
                        stream,
                        home,
                        cache,
                        timestamps,
                        sync_tx,
                        shutdown_tx,
                        started_at_unix,
                    ).await {
                        tracing::error!(error = %err, "socket client error");
                    }
                });
            }
        }
    }

    if socket.exists() {
        let _ = fs::remove_file(&socket);
    }
    Ok(())
}

async fn handle_socket_client(
    stream: UnixStream,
    home: PathBuf,
    cache: std::sync::Arc<RwLock<RegistryCache>>,
    timestamps: std::sync::Arc<RwLock<SyncTimestamps>>,
    sync_tx: mpsc::Sender<SyncJob>,
    shutdown_tx: broadcast::Sender<()>,
    started_at_unix: u64,
) -> Result<(), DaemonError> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| io_err("daemon socket read", e))?
    {
        if line.trim().is_empty() {
            continue;
        }

        let request: Result<DaemonRequest, _> = serde_json::from_str(&line);
        let request = match request {
            Ok(request) => request,
            Err(err) => {
                write_response(
                    &mut writer,
                    &DaemonResponse::error(format!("invalid request JSON: {err}")),
                )
                .await?;
                continue;
            }
        };

        let cmd = request.cmd.clone();
        let codebase = request.codebase.clone();

        let response = match cmd.as_str() {
            "status" => {
                let payload =
                    build_status_payload(&home, cache.clone(), timestamps.clone(), started_at_unix)
                        .await;
                DaemonResponse::ok(payload)
            }
            "sync" => {
                let target = match codebase {
                    Some(codebase) => SyncTarget::Codebase(codebase),
                    None => SyncTarget::All,
                };
                match enqueue_sync(&sync_tx, target, "socket").await {
                    Ok(summary) => DaemonResponse::ok(json!(summary)),
                    Err(err) => DaemonResponse::error(err.to_string()),
                }
            }
            "stop" => {
                let _ = shutdown_tx.send(());
                DaemonResponse::ok(json!({ "stopping": true }))
            }
            other => DaemonResponse::error(format!("unknown command '{other}'")),
        };

        write_response(&mut writer, &response).await?;
        if cmd == "stop" {
            break;
        }
    }

    Ok(())
}

async fn build_status_payload(
    home: &Path,
    cache: std::sync::Arc<RwLock<RegistryCache>>,
    timestamps: std::sync::Arc<RwLock<SyncTimestamps>>,
    started_at_unix: u64,
) -> Value {
    // Collect codebase names from registry cache (read lock, dropped immediately).
    let names: Vec<String> = {
        let cache = cache.read().await;
        let mut v: Vec<String> = cache.keys().map(|name| name.0.clone()).collect();
        v.sort();
        v
    };

    // Snapshot timestamps (read lock, dropped before JSON assembly).
    let ts_snapshot: HashMap<String, u64> = {
        let ts = timestamps.read().await;
        ts.clone()
    };

    // Build per-codebase objects with last sync time and task count.
    let codebases: Vec<Value> = names
        .iter()
        .map(|name| {
            let last_sync = ts_snapshot.get(name).copied().unwrap_or(0);
            json!({
                "name": name,
                "last_sync_at_unix": last_sync,
            })
        })
        .collect();

    // Daemon-wide last sync = max of per-codebase timestamps (0 if none yet).
    let last_sync_at_unix = ts_snapshot.values().copied().max().unwrap_or(0);

    json!({
        "running": true,
        "label": crate::paths::DAEMON_LABEL,
        "started_at_unix": started_at_unix,
        "last_sync_at_unix": last_sync_at_unix,
        "codebases": codebases,
        "socket": socket_path(home).display().to_string(),
        "projects_root": projects_root(home).display().to_string(),
    })
}

async fn enqueue_sync(
    sync_tx: &mpsc::Sender<SyncJob>,
    target: SyncTarget,
    source: &'static str,
) -> Result<SyncSummary, DaemonError> {
    let (tx, rx) = oneshot::channel();
    sync_tx
        .send(SyncJob {
            target,
            source,
            respond_to: tx,
        })
        .await
        .map_err(|_| DaemonError::ChannelClosed("sync queue"))?;

    let outcome = rx
        .await
        .map_err(|_| DaemonError::ChannelClosed("sync response"))?;
    outcome.map_err(DaemonError::Protocol)
}

async fn refresh_cache(
    home: PathBuf,
    cache: std::sync::Arc<RwLock<RegistryCache>>,
) -> Result<(), DaemonError> {
    let refreshed = tokio::task::spawn_blocking(move || load_registry_cache(&home))
        .await
        .map_err(|err| DaemonError::Protocol(format!("cache refresh join error: {err}")))??;
    let mut guard = cache.write().await;
    *guard = refreshed;
    Ok(())
}

async fn log_rotation_task(
    home: PathBuf,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), DaemonError> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    // Skip the first (immediate) tick to avoid rotating on startup.
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await; // consume the first immediate tick

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break,
            _ = interval.tick() => {
                let home = home.clone();
                tokio::task::spawn_blocking(move || {
                    crate::log_rotation::rotate_logs(&home);
                })
                .await
                .ok(); // rotation errors are logged inside rotate_logs; never crash the daemon
            }
        }
    }
    Ok(())
}

async fn run_staleness_scan(home: PathBuf) -> Result<(), DaemonError> {
    tokio::task::spawn_blocking(move || run_staleness_scan_blocking(&home))
        .await
        .map_err(|err| DaemonError::Protocol(format!("staleness scan join error: {err}")))?
}

fn run_staleness_scan_blocking(home: &Path) -> Result<(), DaemonError> {
    let codebases = registry::list_codebases_at(home)?;
    for (project, codebase) in codebases {
        let signal = staleness::check(home, &project, &codebase)?;
        tracing::info!(
            codebase = %codebase.name.0,
            signal = ?signal,
            "staleness signal after watcher sync",
        );
    }
    Ok(())
}

fn build_sync_summary(
    target: SyncTarget,
    source: &'static str,
    results: Vec<SyncCodebaseResult>,
    duration: Duration,
) -> SyncSummary {
    let mut codebases = Vec::new();
    let mut written = 0usize;
    let mut unchanged = 0usize;

    for result in results {
        codebases.push(result.codebase_name);
        for write in result.writes {
            match write {
                WriteResult::Written { .. } | WriteResult::WouldWrite { .. } => written += 1,
                WriteResult::Unchanged { .. } => unchanged += 1,
            }
        }
    }

    SyncSummary {
        target: target.label(),
        source: source.to_string(),
        codebases,
        written,
        unchanged,
        duration_ms: duration.as_millis(),
    }
}

fn load_registry_cache(home: &Path) -> Result<RegistryCache, DaemonError> {
    let mut cache = HashMap::new();
    for (_project, codebase) in registry::list_codebases_at(home)? {
        cache.insert(codebase.name.clone(), codebase);
    }
    Ok(cache)
}

#[cfg(test)]
fn reload_codebase(
    home: &Path,
    cache: &mut RegistryCache,
    codebase_name: &str,
) -> Result<(), DaemonError> {
    let target = CodebaseName::from(codebase_name);
    for (_project, codebase) in registry::list_codebases_at(home)? {
        if codebase.name == target {
            cache.insert(codebase.name.clone(), codebase);
            return Ok(());
        }
    }
    cache.remove(&target);
    Ok(())
}

fn register_projects_tree(
    watcher: &mut RecommendedWatcher,
    watched_dirs: &mut HashSet<PathBuf>,
    root: &Path,
) -> Result<(), DaemonError> {
    if !root.exists() {
        fs::create_dir_all(root).map_err(|e| io_err(root, e))?;
    }
    for dir in collect_dirs(root)? {
        let canonical = match fs::canonicalize(&dir) {
            Ok(path) => path,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => return Err(io_err(&dir, err)),
        };
        if watched_dirs.insert(canonical.clone()) {
            watcher.watch(&canonical, RecursiveMode::NonRecursive)?;
            tracing::debug!(path = %canonical.display(), "watching registry directory");
        }
    }
    Ok(())
}

fn collect_dirs(root: &Path) -> Result<Vec<PathBuf>, DaemonError> {
    let mut dirs = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < dirs.len() {
        let current = dirs[cursor].clone();
        cursor += 1;
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                return Err(io_err(&current, err));
            }
        };
        for entry in entries {
            let entry = entry.map_err(|e| io_err(&current, e))?;
            let ty = entry.file_type().map_err(|e| io_err(entry.path(), e))?;
            if ty.is_dir() {
                dirs.push(entry.path());
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    Ok(dirs)
}

fn is_relevant_event_kind(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Create(_) | EventKind::Modify(_))
}

fn is_registry_yaml(path: &Path, projects: &Path) -> bool {
    path.starts_with(projects)
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("yaml"))
            .unwrap_or(false)
}

fn sync_target_for_path(path: &Path) -> SyncTarget {
    if path.file_name().and_then(|name| name.to_str()) == Some("project.yaml") {
        return SyncTarget::All;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|name| SyncTarget::Codebase(name.to_string()))
        .unwrap_or(SyncTarget::All)
}

fn directory_to_watch(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        path.parent().map(Path::to_path_buf)
    }
}

fn prepare_socket_for_bind(socket: &Path) -> Result<(), DaemonError> {
    if !socket.exists() {
        return Ok(());
    }

    match StdUnixStream::connect(socket) {
        Ok(_) => {
            return Err(DaemonError::Protocol(format!(
                "daemon socket already in use: {}",
                socket.display()
            )));
        }
        Err(err) => {
            tracing::warn!(
                socket = %socket.display(),
                error = %err,
                "removing stale daemon socket before bind",
            );
        }
    }

    match fs::remove_file(socket) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(io_err(socket, err)),
    }
}

fn should_process_event(
    debounce: &mut HashMap<PathBuf, Instant>,
    path: &Path,
    now: Instant,
) -> bool {
    should_process_event_with_threshold(debounce, path, now, DEBOUNCE_WINDOW)
}

fn should_process_event_with_threshold(
    debounce: &mut HashMap<PathBuf, Instant>,
    path: &Path,
    now: Instant,
    threshold: Duration,
) -> bool {
    debounce.retain(|_, seen_at| now.duration_since(*seen_at) <= Duration::from_secs(30));
    match debounce.get(path) {
        Some(last_seen) if now.duration_since(*last_seen) < threshold => false,
        _ => {
            debounce.insert(path.to_path_buf(), now);
            true
        }
    }
}

fn ensure_runtime_dirs(home: &Path) -> Result<(), DaemonError> {
    let projects = projects_root(home);
    if !projects.exists() {
        fs::create_dir_all(&projects).map_err(|e| io_err(&projects, e))?;
    }
    let run = run_dir(home);
    if !run.exists() {
        fs::create_dir_all(&run).map_err(|e| io_err(&run, e))?;
    }
    let logs = crate::paths::logs_dir(home);
    if !logs.exists() {
        fs::create_dir_all(&logs).map_err(|e| io_err(&logs, e))?;
    }
    Ok(())
}

async fn write_response(
    writer: &mut OwnedWriteHalf,
    response: &DaemonResponse,
) -> Result<(), DaemonError> {
    let payload = serde_json::to_string(response)?;
    writer
        .write_all(payload.as_bytes())
        .await
        .map_err(|e| io_err("daemon socket write", e))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|e| io_err("daemon socket write", e))?;
    writer
        .flush()
        .await
        .map_err(|e| io_err("daemon socket flush", e))?;
    Ok(())
}

fn handle_join(
    task: &str,
    result: Result<Result<(), DaemonError>, tokio::task::JoinError>,
) -> Result<(), DaemonError> {
    match result {
        Ok(inner) => inner,
        Err(err) => Err(DaemonError::Protocol(format!(
            "{task} task join failure: {err}"
        ))),
    }
}

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}

#[cfg(unix)]
fn set_socket_permissions(path: &Path) -> Result<(), DaemonError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| io_err(path, e))
}

#[cfg(not(unix))]
fn set_socket_permissions(_path: &Path) -> Result<(), DaemonError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use orchestra_core::types::{ProjectName, ProjectType};
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::sync::{broadcast, mpsc, RwLock};
    use tokio::time::advance;

    #[tokio::test(start_paused = true, flavor = "current_thread")]
    async fn debounce_coalesces_rapid_events() {
        let threshold = Duration::from_millis(100);
        let mut debounce = HashMap::<PathBuf, Instant>::new();
        let path = PathBuf::from("/tmp/copenow_api.yaml");
        let mut sync_triggers = 0usize;

        for _ in 0..5 {
            if should_process_event_with_threshold(&mut debounce, &path, Instant::now(), threshold)
            {
                sync_triggers += 1;
            }
            advance(Duration::from_millis(10)).await;
        }

        advance(Duration::from_millis(150)).await;
        assert_eq!(
            sync_triggers, 1,
            "rapid saves should collapse to one sync trigger"
        );
    }

    #[test]
    fn registry_cache_reload_updates_changed_codebase() {
        let home = TempDir::new().expect("home");
        let workspace = TempDir::new().expect("workspace");
        let project = ProjectName::from("copnow");

        for name in ["core_api", "copnow_api", "worker_api"] {
            let path = workspace.path().join(name);
            fs::create_dir_all(&path).expect("create codebase dir");
            registry::init_at(
                path,
                project.clone(),
                Some(ProjectType::Backend),
                home.path(),
            )
            .expect("init codebase");
        }

        let mut cache = load_registry_cache(home.path()).expect("load cache");
        assert_eq!(
            cache.len(),
            3,
            "cache should contain all registered codebases"
        );

        let mut codebase =
            registry::load_codebase_at(home.path(), &project, &CodebaseName::from("copnow_api"))
                .expect("load codebase");
        codebase.projects[0].name = ProjectName::from("phase04-cache-reload");
        registry::save_codebase_at(home.path(), &project, &codebase).expect("save codebase");

        reload_codebase(home.path(), &mut cache, "copnow_api").expect("reload cache entry");
        let reloaded = cache
            .get(&CodebaseName::from("copnow_api"))
            .expect("codebase in cache");
        assert_eq!(reloaded.projects[0].name.0, "phase04-cache-reload");
    }

    #[tokio::test]
    async fn socket_protocol_status_and_stop_over_in_memory_channels() {
        let (request_tx, mut request_rx) = mpsc::channel::<Vec<u8>>(8);
        let (response_tx, mut response_rx) = mpsc::channel::<Vec<u8>>(8);
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

        tokio::spawn(async move {
            while let Some(bytes) = request_rx.recv().await {
                let line = String::from_utf8(bytes).expect("utf8");
                let request: DaemonRequest = serde_json::from_str(line.trim()).expect("request");
                let response = match request.cmd.as_str() {
                    "status" => DaemonResponse::ok(json!({"running": true})),
                    "stop" => {
                        let _ = shutdown_tx.send(());
                        DaemonResponse::ok(json!({"stopping": true}))
                    }
                    other => DaemonResponse::error(format!("unknown command '{other}'")),
                };
                let encoded = serde_json::to_vec(&response).expect("encode response");
                if response_tx.send(encoded).await.is_err() {
                    break;
                }
            }
        });

        request_tx
            .send(br#"{"cmd":"status"}"#.to_vec())
            .await
            .expect("send status request");
        let status_response = response_rx.recv().await.expect("status response");
        let status_json: serde_json::Value =
            serde_json::from_slice(&status_response).expect("decode status");
        assert_eq!(status_json["ok"], serde_json::Value::Bool(true));

        request_tx
            .send(br#"{"cmd":"stop"}"#.to_vec())
            .await
            .expect("send stop request");
        let stop_response = response_rx.recv().await.expect("stop response");
        let stop_json: serde_json::Value =
            serde_json::from_slice(&stop_response).expect("decode stop");
        assert_eq!(stop_json["ok"], serde_json::Value::Bool(true));

        shutdown_rx.recv().await.expect("shutdown signal");
    }

    // ─── Status payload tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn status_payload_has_last_sync_at_unix_when_never_synced() {
        let home = TempDir::new().expect("home");
        let cache = std::sync::Arc::new(RwLock::new(RegistryCache::new()));
        let timestamps = std::sync::Arc::new(RwLock::new(SyncTimestamps::new()));

        let payload = build_status_payload(home.path(), cache, timestamps, 1_000_000).await;

        assert_eq!(payload["running"], json!(true));
        assert_eq!(payload["started_at_unix"], json!(1_000_000u64));
        assert_eq!(
            payload["last_sync_at_unix"],
            json!(0u64),
            "should be 0 before any sync"
        );
        let codebases = payload["codebases"].as_array().expect("codebases array");
        assert!(codebases.is_empty(), "empty codebases when cache is empty");
    }

    #[tokio::test]
    async fn status_payload_includes_per_codebase_last_sync_timestamps() {
        let home = TempDir::new().expect("home");
        let workspace = TempDir::new().expect("workspace");
        let project = ProjectName::from("copnow");

        for name in ["api", "worker"] {
            let path = workspace.path().join(name);
            fs::create_dir_all(&path).expect("create codebase dir");
            registry::init_at(
                path,
                project.clone(),
                Some(ProjectType::Backend),
                home.path(),
            )
            .expect("init codebase");
        }

        let cache = std::sync::Arc::new(RwLock::new(
            load_registry_cache(home.path()).expect("load cache"),
        ));

        let ts_map: SyncTimestamps = [
            ("api".to_string(), 1_000_100u64),
            ("worker".to_string(), 1_000_200u64),
        ]
        .into_iter()
        .collect();
        let timestamps = std::sync::Arc::new(RwLock::new(ts_map));

        let payload = build_status_payload(home.path(), cache, timestamps, 1_000_000).await;

        // Daemon-wide last sync = max of the two.
        assert_eq!(
            payload["last_sync_at_unix"],
            json!(1_000_200u64),
            "daemon-wide last_sync should be the max codebase timestamp"
        );

        // Per-codebase objects must have name + last_sync_at_unix.
        let codebases = payload["codebases"].as_array().expect("codebases array");
        assert_eq!(codebases.len(), 2, "two codebases expected");

        for cb in codebases {
            let name = cb["name"].as_str().expect("name field");
            let ts = cb["last_sync_at_unix"].as_u64().expect("timestamp field");
            match name {
                "api" => assert_eq!(ts, 1_000_100, "api timestamp mismatch"),
                "worker" => assert_eq!(ts, 1_000_200, "worker timestamp mismatch"),
                other => panic!("unexpected codebase name: {other}"),
            }
        }
    }

    #[tokio::test]
    async fn sync_processor_records_codebase_timestamps_on_success() {
        // Simulate what sync_processor_task does after a successful sync.
        let timestamps = std::sync::Arc::new(RwLock::new(SyncTimestamps::new()));

        let before = unix_seconds_now();
        {
            let mut ts = timestamps.write().await;
            ts.insert("copnow_api".to_string(), unix_seconds_now());
        }
        let after = unix_seconds_now();

        let ts = timestamps.read().await;
        let recorded = *ts.get("copnow_api").expect("timestamp recorded");
        assert!(
            recorded >= before && recorded <= after,
            "recorded timestamp {recorded} should be between {before} and {after}"
        );
    }
}
