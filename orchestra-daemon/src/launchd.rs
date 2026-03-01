use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{io_err, DaemonError};
use crate::paths::{launch_agents_dir, launchd_plist_path, socket_path, DAEMON_LABEL};

/// Generate a launchd plist for Orchestra daemon management.
pub fn generate_plist(binary_path: &Path, log_dir: &Path) -> String {
    let stdout = log_dir.join("daemon.log").display().to_string();
    let stderr = log_dir.join("daemon-err.log").display().to_string();
    let binary = binary_path.display().to_string();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{binary}</string>
    <string>daemon</string>
    <string>start</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{stdout}</string>
  <key>StandardErrorPath</key>
  <string>{stderr}</string>
</dict>
</plist>
"#,
        label = DAEMON_LABEL,
        binary = binary,
        stdout = stdout,
        stderr = stderr
    )
}

/// Install and bootstrap launchd service for the current user.
pub fn install(home: &Path) -> Result<PathBuf, DaemonError> {
    ensure_macos()?;

    let launch_agents = launch_agents_dir(home);
    if !launch_agents.exists() {
        fs::create_dir_all(&launch_agents).map_err(|e| io_err(&launch_agents, e))?;
    }

    let logs = crate::paths::logs_dir(home);
    if !logs.exists() {
        fs::create_dir_all(&logs).map_err(|e| io_err(&logs, e))?;
    }
    let run = crate::paths::run_dir(home);
    if !run.exists() {
        fs::create_dir_all(&run).map_err(|e| io_err(&run, e))?;
    }

    let plist = launchd_plist_path(home);
    let binary_path = Path::new("/usr/local/bin/orchestra");
    fs::write(&plist, generate_plist(binary_path, &logs)).map_err(|e| io_err(&plist, e))?;

    let domain = launchctl_domain()?;
    let service = format!("{domain}/{DAEMON_LABEL}");

    let _ = run_launchctl(vec!["bootout".to_string(), service.clone()], true);
    run_launchctl(
        vec![
            "bootstrap".to_string(),
            domain.clone(),
            plist.display().to_string(),
        ],
        false,
    )?;
    run_launchctl(
        vec!["kickstart".to_string(), "-k".to_string(), service],
        false,
    )?;

    Ok(plist)
}

/// Boot out launchd service and remove plist.
pub fn uninstall(home: &Path) -> Result<(), DaemonError> {
    ensure_macos()?;

    let plist = launchd_plist_path(home);
    if plist.exists() {
        let domain = launchctl_domain()?;
        let service = format!("{domain}/{DAEMON_LABEL}");
        let _ = run_launchctl(vec!["bootout".to_string(), service], true);
        fs::remove_file(&plist).map_err(|e| io_err(&plist, e))?;
    }

    let socket = socket_path(home);
    if socket.exists() {
        let _ = fs::remove_file(socket);
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn ensure_macos() -> Result<(), DaemonError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn ensure_macos() -> Result<(), DaemonError> {
    Err(DaemonError::Launchd(
        "launchd management is only supported on macOS".to_string(),
    ))
}

fn run_launchctl(args: Vec<String>, ignore_failure: bool) -> Result<(), DaemonError> {
    let output = Command::new("launchctl")
        .args(args.iter().map(String::as_str))
        .output()
        .map_err(|e| io_err("launchctl", e))?;

    if output.status.success() || ignore_failure {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(DaemonError::Launchd(format!(
        "launchctl failed (status {}): {} {}",
        output.status, stdout, stderr
    )))
}

fn launchctl_domain() -> Result<String, DaemonError> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|e| io_err("id -u", e))?;
    if !output.status.success() {
        return Err(DaemonError::Launchd(format!(
            "failed to resolve current uid (status {})",
            output.status
        )));
    }

    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uid.is_empty() {
        return Err(DaemonError::Launchd(
            "current uid from `id -u` was empty".to_string(),
        ));
    }
    Ok(format!("gui/{uid}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plist::Value;

    #[test]
    fn plist_contains_required_launchd_fields() {
        let binary = Path::new("/usr/local/bin/orchestra");
        let log_dir = Path::new("/Users/tester/.orchestra/logs");
        let plist = generate_plist(binary, log_dir);

        let value = Value::from_reader_xml(plist.as_bytes()).expect("parse plist");
        let dict = value.as_dictionary().expect("plist root dict");

        assert_eq!(
            dict.get("Label").and_then(Value::as_string),
            Some("dev.orchestra.daemon")
        );
        assert_eq!(
            dict.get("RunAtLoad").and_then(Value::as_boolean),
            Some(true)
        );
        assert_eq!(
            dict.get("KeepAlive").and_then(Value::as_boolean),
            Some(true)
        );

        let args = dict
            .get("ProgramArguments")
            .and_then(Value::as_array)
            .expect("ProgramArguments array");
        let rendered_args: Vec<&str> = args
            .iter()
            .map(|v| v.as_string().expect("program arg as string"))
            .collect();
        assert_eq!(
            rendered_args,
            vec!["/usr/local/bin/orchestra", "daemon", "start"]
        );
    }
}
