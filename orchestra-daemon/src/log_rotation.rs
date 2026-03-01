//! Size-based log rotation for daemon log files.
//!
//! Rotates `daemon.log` and `daemon-err.log` when they exceed 10 MiB.
//! Keeps at most 5 rotated copies using the scheme:
//!   daemon.log → daemon.log.1 → daemon.log.2 → … → daemon.log.5

use std::fs;
use std::io;
use std::path::Path;

/// Maximum log file size before rotation (10 MiB).
pub const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

/// Maximum number of rotated backup files to keep.
pub const MAX_ROTATED_FILES: usize = 5;

/// Rotate `log_path` if its size exceeds `max_bytes`.
///
/// Rotation sequence (oldest first):
///   `<name>.<max_files>` deleted  
///   `<name>.<n>` → `<name>.<n+1>` for n = max_files-1 … 1  
///   `<name>` → `<name>.1`  
///   Create fresh empty `<name>`.
///
/// Returns `true` if rotation occurred, `false` if the file was under the
/// threshold (or did not exist yet).
///
/// # Errors
/// Returns `io::Error` only on unexpected filesystem failures; missing files
/// are silently skipped.
pub fn rotate_if_needed(
    log_path: &Path,
    max_bytes: u64,
    max_files: usize,
) -> io::Result<bool> {
    let size = match fs::metadata(log_path) {
        Ok(meta) => meta.len(),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };

    if size < max_bytes {
        return Ok(false);
    }

    // Remove the oldest file so we don't exceed max_files.
    let oldest = numbered_path(log_path, max_files);
    if oldest.exists() {
        fs::remove_file(&oldest)?;
    }

    // Shift existing rotated files up by one.
    for n in (1..max_files).rev() {
        let src = numbered_path(log_path, n);
        let dst = numbered_path(log_path, n + 1);
        if src.exists() {
            fs::rename(&src, &dst)?;
        }
    }

    // Rename live log → .1
    fs::rename(log_path, numbered_path(log_path, 1))?;

    // Create fresh empty log file so the daemon always has a writable path.
    let _ = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(log_path)?;

    Ok(true)
}

/// Rotate both `daemon.log` and `daemon-err.log` under `home`.
///
/// Errors for one file are logged as warnings and do not block the other.
pub fn rotate_logs(home: &Path) {
    let stdout_log = crate::paths::stdout_log_path(home);
    let stderr_log = crate::paths::stderr_log_path(home);

    for log_path in [&stdout_log, &stderr_log] {
        match rotate_if_needed(log_path, MAX_LOG_BYTES, MAX_ROTATED_FILES) {
            Ok(true) => tracing::info!(path = %log_path.display(), "log file rotated"),
            Ok(false) => {}
            Err(err) => tracing::warn!(path = %log_path.display(), error = %err, "log rotation failed"),
        }
    }
}

/// Build the path for the `n`-th rotated copy of `base` (e.g. `daemon.log.2`).
fn numbered_path(base: &Path, n: usize) -> std::path::PathBuf {
    let name = base
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("daemon.log");
    base.with_file_name(format!("{name}.{n}"))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_log(dir: &TempDir, name: &str, size_bytes: usize) -> std::path::PathBuf {
        let path = dir.path().join(name);
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        // Write in 64 KiB chunks to avoid huge allocations in tests.
        let chunk = vec![b'x'; 64 * 1024];
        let mut written = 0usize;
        while written < size_bytes {
            let to_write = (size_bytes - written).min(chunk.len());
            f.write_all(&chunk[..to_write]).unwrap();
            written += to_write;
        }
        path
    }

    #[test]
    fn rotation_noop_when_file_under_threshold() {
        let dir = TempDir::new().unwrap();
        let log = make_log(&dir, "daemon.log", 1024); // 1 KiB
        let rotated = rotate_if_needed(&log, MAX_LOG_BYTES, MAX_ROTATED_FILES).unwrap();
        assert!(!rotated, "should not rotate a small file");
        assert!(!numbered_path(&log, 1).exists(), "no .1 file should exist");
    }

    #[test]
    fn rotation_triggers_when_file_exceeds_max_bytes() {
        let dir = TempDir::new().unwrap();
        // 10 MiB + 1 byte
        let log = make_log(&dir, "daemon.log", MAX_LOG_BYTES as usize + 1);
        let rotated = rotate_if_needed(&log, MAX_LOG_BYTES, MAX_ROTATED_FILES).unwrap();
        assert!(rotated, "should rotate an oversized file");

        // Original log exists and is empty.
        let size = fs::metadata(&log).unwrap().len();
        assert_eq!(size, 0, "rotated log should be empty");

        // First rotated copy exists and has the original content.
        let backup = numbered_path(&log, 1);
        assert!(backup.exists(), "daemon.log.1 should exist");
        let backup_size = fs::metadata(&backup).unwrap().len();
        assert!(backup_size > 0, "backup should have content");
    }

    #[test]
    fn max_rotated_files_are_capped() {
        let dir = TempDir::new().unwrap();
        let log = dir.path().join("daemon.log");

        // Pre-create .1 through .5 filled files.
        for n in 1..=MAX_ROTATED_FILES {
            let p = numbered_path(&log, n);
            fs::write(&p, format!("rotated-{n}")).unwrap();
        }

        // Write an oversized live file.
        make_log(&dir, "daemon.log", MAX_LOG_BYTES as usize + 1);

        let rotated = rotate_if_needed(&log, MAX_LOG_BYTES, MAX_ROTATED_FILES).unwrap();
        assert!(rotated);

        // .5 exists (was .4 before rotation), .6 must NOT exist.
        assert!(numbered_path(&log, MAX_ROTATED_FILES).exists());
        assert!(
            !numbered_path(&log, MAX_ROTATED_FILES + 1).exists(),
            "must not create more than MAX_ROTATED_FILES backup files"
        );
    }

    #[test]
    fn rotation_skips_missing_file_gracefully() {
        let dir = TempDir::new().unwrap();
        let log = dir.path().join("nonexistent.log");
        // Should not error, should return false.
        let rotated = rotate_if_needed(&log, MAX_LOG_BYTES, MAX_ROTATED_FILES).unwrap();
        assert!(!rotated);
    }

    #[test]
    fn sequential_rotations_shift_files_correctly() {
        let dir = TempDir::new().unwrap();
        let log = dir.path().join("daemon.log");

        // Run three rotations; each time write a fresh large file.
        for round in 1..=3usize {
            fs::write(&log, vec![b'0' + round as u8; MAX_LOG_BYTES as usize + 1]).unwrap();
            rotate_if_needed(&log, MAX_LOG_BYTES, MAX_ROTATED_FILES).unwrap();
        }

        // After 3 rotations: .1 (newest), .2, .3 exist; original is empty.
        for n in 1..=3 {
            assert!(
                numbered_path(&log, n).exists(),
                "backup .{n} should exist after 3 rotations"
            );
        }
        assert!(!numbered_path(&log, 4).exists());
    }
}
