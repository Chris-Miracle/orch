//! `orchestra daemon` — background watcher lifecycle and launchd management.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use orchestra_daemon::paths::{socket_path, stderr_log_path, stdout_log_path};
use orchestra_daemon::{
    install_launchd, request_status, request_stop, start_blocking, uninstall_launchd, DaemonError,
};

#[derive(Subcommand, Debug)]
pub enum DaemonCommand {
    /// Run daemon in foreground (watcher + socket server).
    Start,
    /// Request graceful daemon shutdown over Unix socket.
    Stop,
    /// Query daemon runtime status over Unix socket.
    Status,
    /// Install and bootstrap launchd agent.
    Install,
    /// Boot out and remove launchd agent.
    Uninstall,
    /// Print recent daemon log lines.
    Logs(DaemonLogsArgs),
}

#[derive(Args, Debug)]
pub struct DaemonLogsArgs {
    /// Number of trailing lines to show.
    #[arg(long, default_value_t = 100)]
    pub lines: usize,

    /// Show only stderr log file.
    #[arg(long)]
    pub stderr_only: bool,
}

pub fn run(command: DaemonCommand) -> Result<()> {
    let home = dirs::home_dir().context("could not determine home directory")?;

    match command {
        DaemonCommand::Start => {
            start_blocking(&home).context("daemon exited with error")?;
        }
        DaemonCommand::Stop => match request_stop(&home) {
            Ok(()) => println!("daemon stop requested"),
            Err(DaemonError::DaemonNotRunning { .. }) => {
                println!("daemon is not running");
            }
            Err(err) => return Err(err).context("failed to stop daemon"),
        },
        DaemonCommand::Status => match request_status(&home) {
            Ok(status) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&status)
                        .context("failed to render daemon status JSON")?
                );
            }
            Err(DaemonError::DaemonNotRunning { .. }) => {
                let payload = serde_json::json!({
                    "running": false,
                    "socket": socket_path(&home).display().to_string(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .context("failed to render daemon status JSON")?
                );
            }
            Err(err) => return Err(err).context("failed to query daemon status"),
        },
        DaemonCommand::Install => {
            let path = install_launchd(&home).context("failed to install launchd service")?;
            println!("installed launchd service: {}", path.display());
        }
        DaemonCommand::Uninstall => {
            uninstall_launchd(&home).context("failed to uninstall launchd service")?;
            println!("uninstalled launchd service");
        }
        DaemonCommand::Logs(args) => {
            if args.stderr_only {
                print_tail(&stderr_log_path(&home), args.lines)
                    .context("failed to read daemon stderr log")?;
            } else {
                print_tail(&stdout_log_path(&home), args.lines)
                    .context("failed to read daemon stdout log")?;
                print_tail(&stderr_log_path(&home), args.lines)
                    .context("failed to read daemon stderr log")?;
            }
        }
    }

    Ok(())
}

fn print_tail(path: &std::path::Path, lines: usize) -> Result<()> {
    if !path.exists() {
        println!("log file not found: {}", path.display());
        return Ok(());
    }

    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut tail = VecDeque::<String>::new();
    for line in reader.lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        if tail.len() == lines {
            tail.pop_front();
        }
        tail.push_back(line);
    }

    println!("==> {} <==", path.display());
    for line in tail {
        println!("{line}");
    }
    Ok(())
}
