//! `orchestra doctor` — broad health diagnostics.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use serde::Serialize;

use orchestra_core::registry;
use orchestra_daemon::{paths::socket_path, request_status, DaemonError};
use orchestra_sync::{managed_agent_paths, staleness};

const REPO: &str = "Chris-Miracle/orch";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DoctorCheck {
    name: String,
    status: String,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
struct DoctorReport {
    version: String,
    checks: Vec<DoctorCheck>,
}

impl DoctorArgs {
    pub fn run(self) -> Result<()> {
        let home: PathBuf = dirs::home_dir().context("could not determine home directory")?;
        let mut checks = Vec::new();

        checks.push(version_check());
        checks.push(path_check());
        checks.push(daemon_socket_check(&home));
        checks.push(daemon_check(&home));

        let codebases_res = registry::list_codebases_at(&home);
        match codebases_res {
            Ok(codebases) => {
                checks.push(DoctorCheck {
                    name: "registry integrity".into(),
                    status: "pass".into(),
                    detail: format!("{} codebase entries loaded", codebases.len()),
                });

                let mut missing_paths = Vec::new();
                let mut missing_pilot = Vec::new();
                let mut stale = 0usize;
                let mut current = 0usize;
                let mut other = 0usize;
                let mut missing_managed = Vec::new();

                for (project, codebase) in &codebases {
                    if !codebase.path.exists() {
                        missing_paths.push(format!("{}:{}", project.0, codebase.path.display()));
                    }

                    let pilot = codebase.path.join(".orchestra").join("pilot.md");
                    if !pilot.exists() {
                        missing_pilot.push(format!("{}:{}", project.0, codebase.name.0));
                    }

                    if let Ok(signal) = staleness::check(&home, project, codebase) {
                        match signal {
                            orchestra_sync::StalenessSignal::Current => current += 1,
                            orchestra_sync::StalenessSignal::Stale { .. } => stale += 1,
                            _ => other += 1,
                        }
                    }

                    let mut expected = managed_agent_paths(&[(project.clone(), codebase.clone())]);
                    expected.push(codebase.path.join(".orchestra").join("pilot.md"));
                    let missing = expected
                        .into_iter()
                        .filter(|p| !p.exists())
                        .count();
                    if missing > 0 {
                        missing_managed.push(format!("{}:{} missing {}", project.0, codebase.name.0, missing));
                    }
                }

                if missing_paths.is_empty() {
                    checks.push(DoctorCheck {
                        name: "codebase paths".into(),
                        status: "pass".into(),
                        detail: "all registered codebase paths exist".into(),
                    });
                } else {
                    checks.push(DoctorCheck {
                        name: "codebase paths".into(),
                        status: "warn".into(),
                        detail: format!("missing: {}", missing_paths.join(", ")),
                    });
                }

                if missing_pilot.is_empty() {
                    checks.push(DoctorCheck {
                        name: "pilot.md presence".into(),
                        status: "pass".into(),
                        detail: "all codebases have .orchestra/pilot.md".into(),
                    });
                } else {
                    checks.push(DoctorCheck {
                        name: "pilot.md presence".into(),
                        status: "warn".into(),
                        detail: format!("missing: {}", missing_pilot.join(", ")),
                    });
                }

                checks.push(DoctorCheck {
                    name: "staleness summary".into(),
                    status: if stale == 0 { "pass".into() } else { "warn".into() },
                    detail: format!("current: {current}, stale: {stale}, other: {other}"),
                });

                if missing_managed.is_empty() {
                    checks.push(DoctorCheck {
                        name: "managed files presence".into(),
                        status: "pass".into(),
                        detail: "all expected managed files are present".into(),
                    });
                } else {
                    checks.push(DoctorCheck {
                        name: "managed files presence".into(),
                        status: "warn".into(),
                        detail: missing_managed.join(", "),
                    });
                }
            }
            Err(err) => {
                checks.push(DoctorCheck {
                    name: "registry integrity".into(),
                    status: "fail".into(),
                    detail: err.to_string(),
                });
            }
        }

        let report = DoctorReport {
            version: CURRENT_VERSION.to_string(),
            checks,
        };

        if self.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).context("failed to serialize doctor JSON")?
            );
            return Ok(());
        }

        print_human(&report);
        Ok(())
    }
}

fn daemon_socket_check(home: &std::path::Path) -> DoctorCheck {
    let socket = socket_path(home);
    if socket.exists() {
        DoctorCheck {
            name: "daemon socket".into(),
            status: "pass".into(),
            detail: socket.display().to_string(),
        }
    } else {
        DoctorCheck {
            name: "daemon socket".into(),
            status: "warn".into(),
            detail: format!("missing: {}", socket.display()),
        }
    }
}

fn print_human(report: &DoctorReport) {
    println!("Orchestra Doctor — v{}", report.version);
    for check in &report.checks {
        let icon = match check.status.as_str() {
            "pass" => "✓".green().bold().to_string(),
            "warn" => "⚠".yellow().bold().to_string(),
            _ => "✗".red().bold().to_string(),
        };
        println!("  {} {}: {}", icon, check.name, check.detail);
    }
}

fn version_check() -> DoctorCheck {
    match fetch_latest_release_tag() {
        Ok(Some(tag)) => {
            if tag.contains(CURRENT_VERSION) {
                DoctorCheck {
                    name: "version update".into(),
                    status: "pass".into(),
                    detail: format!("running latest ({})", tag),
                }
            } else {
                DoctorCheck {
                    name: "version update".into(),
                    status: "warn".into(),
                    detail: format!("newer release available: {}", tag),
                }
            }
        }
        Ok(None) => DoctorCheck {
            name: "version update".into(),
            status: "warn".into(),
            detail: "no release tag found from GitHub API".into(),
        },
        Err(err) => DoctorCheck {
            name: "version update".into(),
            status: "warn".into(),
            detail: format!("could not check release API: {err}"),
        },
    }
}

fn fetch_latest_release_tag() -> Result<Option<String>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let response = ureq::get(&url)
        .set("User-Agent", "orchestra-cli")
        .call()
        .context("GitHub release request failed")?;

    let value: serde_json::Value = response
        .into_json()
        .context("failed to parse release API response")?;
    Ok(value
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}

fn path_check() -> DoctorCheck {
    let path_var = std::env::var_os("PATH");
    let current_exe = std::env::current_exe().ok();

    let Some(path_var) = path_var else {
        return DoctorCheck {
            name: "binary in PATH".into(),
            status: "warn".into(),
            detail: "PATH is not set".into(),
        };
    };

    let Some(exe) = current_exe else {
        return DoctorCheck {
            name: "binary in PATH".into(),
            status: "warn".into(),
            detail: "could not resolve current executable".into(),
        };
    };

    let parent = exe.parent().map(|p| p.to_path_buf());
    let in_path = std::env::split_paths(&path_var)
        .any(|candidate| Some(candidate) == parent);

    if in_path {
        DoctorCheck {
            name: "binary in PATH".into(),
            status: "pass".into(),
            detail: exe.display().to_string(),
        }
    } else {
        DoctorCheck {
            name: "binary in PATH".into(),
            status: "warn".into(),
            detail: format!("{} is not in PATH entries", exe.display()),
        }
    }
}

fn daemon_check(home: &std::path::Path) -> DoctorCheck {
    match request_status(home) {
        Ok(status) => {
            let running = status
                .get("running")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            DoctorCheck {
            name: "daemon status".into(),
                status: if running { "pass".into() } else { "warn".into() },
                detail: format!("running: {}", running),
            }
        }
        Err(DaemonError::DaemonNotRunning { .. }) => DoctorCheck {
            name: "daemon status".into(),
            status: "warn".into(),
            detail: "daemon is not running".into(),
        },
        Err(err) => DoctorCheck {
            name: "daemon status".into(),
            status: "warn".into(),
            detail: format!("unable to query daemon: {err}"),
        },
    }
}
