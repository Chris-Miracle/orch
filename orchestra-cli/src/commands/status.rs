//! `orchestra status` — staleness and sync visibility.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use serde::Serialize;
use tabled::{settings::Style, Table, Tabled};

use orchestra_core::{registry, types::TaskStatus};
use orchestra_sync::{
    hash_store,
    staleness::{check, format_datetime_age},
    StalenessSignal,
};

/// Arguments for `orchestra status`.
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Filter to a specific registry project.
    #[arg(long)]
    pub project: Option<String>,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

impl StatusArgs {
    pub fn run(self) -> Result<()> {
        let home: PathBuf = dirs::home_dir().context("could not determine home directory")?;

        let mut codebases = registry::list_codebases_at(&home)
            .context("failed to load registry — run `orchestra init` first")?;
        if let Some(project_filter) = self.project.as_ref() {
            codebases.retain(|(project, _)| project.0 == *project_filter);
        }

        let report = build_report(&home, &codebases)?;
        if self.json {
            print_json(report)?;
            return Ok(());
        }

        print_table(report);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct CodebaseStatus {
    project: String,
    codebase: String,
    signal: StalenessSignal,
    detail: String,
    last_sync_age: String,
    last_sync_at: Option<String>,
    active_tasks: usize,
}

#[derive(Debug, Clone)]
struct StatusReport {
    project_count: usize,
    stale_count: usize,
    needs_sync_count: usize,
    codebases: Vec<CodebaseStatus>,
}

#[derive(Serialize)]
struct StatusReportJson {
    summary: StatusSummaryJson,
    codebases: Vec<CodebaseStatusJson>,
}

#[derive(Serialize)]
struct StatusSummaryJson {
    projects: usize,
    codebases: usize,
    stale: usize,
}

#[derive(Serialize)]
struct CodebaseStatusJson {
    project: String,
    codebase: String,
    status: String,
    detail: String,
    last_sync_age: String,
    last_sync_at: Option<String>,
    active_tasks: usize,
}

#[derive(Tabled)]
struct StatusTableRow {
    #[tabled(rename = "codebase")]
    codebase: String,
    #[tabled(rename = "status")]
    status: String,
    #[tabled(rename = "detail")]
    detail: String,
    #[tabled(rename = "last sync")]
    last_sync: String,
    #[tabled(rename = "active tasks")]
    active_tasks: usize,
}

fn build_report(
    home: &Path,
    codebases: &[(
        orchestra_core::types::ProjectName,
        orchestra_core::types::Codebase,
    )],
) -> Result<StatusReport> {
    let project_count = codebases
        .iter()
        .map(|(project, _)| project.0.clone())
        .collect::<BTreeSet<_>>()
        .len();

    let mut rows = Vec::new();
    for (project, codebase) in codebases {
        let signal = check(home, project, codebase)
            .with_context(|| format!("status check failed for '{}'", codebase.name))?;
        let active_tasks = count_active_tasks(codebase);
        let (last_sync_at, last_sync_age) = load_last_sync(home, &codebase.name.0)
            .with_context(|| format!("failed to load hash store for '{}'", codebase.name))?;

        rows.push(CodebaseStatus {
            project: project.0.clone(),
            codebase: codebase.name.0.clone(),
            detail: signal_detail(&signal),
            signal,
            last_sync_age,
            last_sync_at,
            active_tasks,
        });
    }

    let stale_count = rows
        .iter()
        .filter(|r| matches!(r.signal, StalenessSignal::Stale { .. }))
        .count();
    let needs_sync_count = rows
        .iter()
        .filter(|r| !matches!(r.signal, StalenessSignal::Current))
        .count();

    Ok(StatusReport {
        project_count,
        stale_count,
        needs_sync_count,
        codebases: rows,
    })
}

fn load_last_sync(home: &Path, codebase_name: &str) -> Result<(Option<String>, String)> {
    let path = hash_store::store_path_at(home, codebase_name);
    if !path.exists() {
        return Ok((None, "never".to_string()));
    }
    let store = hash_store::load_at(home, codebase_name)?;
    if store.files.is_empty() {
        return Ok((None, "never".to_string()));
    }
    let iso = Some(store.synced_at.to_rfc3339());
    let age = format_datetime_age(store.synced_at);
    Ok((iso, age))
}

fn count_active_tasks(codebase: &orchestra_core::types::Codebase) -> usize {
    codebase
        .projects
        .iter()
        .flat_map(|project| project.tasks.iter())
        .filter(|task| !matches!(task.status, TaskStatus::Done))
        .count()
}

fn print_json(report: StatusReport) -> Result<()> {
    let payload = StatusReportJson {
        summary: StatusSummaryJson {
            projects: report.project_count,
            codebases: report.codebases.len(),
            stale: report.stale_count,
        },
        codebases: report
            .codebases
            .into_iter()
            .map(|row| CodebaseStatusJson {
                project: row.project,
                codebase: row.codebase,
                status: signal_key(&row.signal).to_string(),
                detail: row.detail,
                last_sync_age: row.last_sync_age,
                last_sync_at: row.last_sync_at,
                active_tasks: row.active_tasks,
            })
            .collect(),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).context("failed to serialize status JSON")?
    );
    Ok(())
}

fn print_table(report: StatusReport) {
    println!(
        "Orchestra v{} | {} projects | {} codebases | {} stale",
        env!("CARGO_PKG_VERSION"),
        report.project_count,
        report.codebases.len(),
        report.stale_count,
    );

    if report.codebases.is_empty() {
        println!("No codebases registered.");
        return;
    }

    let separator = "■".repeat(67).bright_black().to_string();
    let mut grouped = BTreeMap::<String, Vec<CodebaseStatus>>::new();
    for row in report.codebases {
        grouped.entry(row.project.clone()).or_default().push(row);
    }

    println!("{separator}");
    println!(
        "Indicators: {} CURRENT  {} STALE  {} MODIFIED  {} ORPHAN  {} NEVER SYNCED",
        signal_indicator(&StalenessSignal::Current),
        signal_indicator(&StalenessSignal::Stale {
            reason: String::new(),
        }),
        signal_indicator(&StalenessSignal::Modified { files: Vec::new() }),
        signal_indicator(&StalenessSignal::Orphan { files: Vec::new() }),
        signal_indicator(&StalenessSignal::NeverSynced),
    );
    println!("{separator}");
    for (project, rows) in grouped {
        println!("{}", project.to_uppercase().bold());
        let table_rows: Vec<StatusTableRow> = rows
            .into_iter()
            .map(|row| StatusTableRow {
                codebase: row.codebase,
                status: signal_label(&row.signal).to_string(),
                detail: row.detail,
                last_sync: row.last_sync_age,
                active_tasks: row.active_tasks,
            })
            .collect();
        let mut table = Table::new(table_rows);
        table.with(Style::rounded());
        println!("{table}");
        println!("{separator}");
    }

    if report.needs_sync_count > 0 {
        println!("Run 'orchestra sync --all' to update stale codebases.");
    }
}

fn signal_key(signal: &StalenessSignal) -> &'static str {
    match signal {
        StalenessSignal::NeverSynced => "never_synced",
        StalenessSignal::Current => "current",
        StalenessSignal::Stale { .. } => "stale",
        StalenessSignal::Modified { .. } => "modified",
        StalenessSignal::Orphan { .. } => "orphan",
    }
}

fn signal_label(signal: &StalenessSignal) -> &'static str {
    match signal {
        StalenessSignal::NeverSynced => "NEVER SYNCED",
        StalenessSignal::Current => "CURRENT",
        StalenessSignal::Stale { .. } => "STALE",
        StalenessSignal::Modified { .. } => "MODIFIED",
        StalenessSignal::Orphan { .. } => "ORPHAN",
    }
}

fn signal_indicator(signal: &StalenessSignal) -> String {
    match signal {
        StalenessSignal::NeverSynced => "■".bright_black().bold().to_string(),
        StalenessSignal::Current => "■".green().bold().to_string(),
        StalenessSignal::Stale { .. } => "■".yellow().bold().to_string(),
        StalenessSignal::Modified { .. } => "■".red().bold().to_string(),
        StalenessSignal::Orphan { .. } => "■".magenta().bold().to_string(),
    }
}

fn signal_detail(signal: &StalenessSignal) -> String {
    match signal {
        StalenessSignal::NeverSynced => "no hash store entries".to_string(),
        StalenessSignal::Current => "up to date".to_string(),
        StalenessSignal::Stale { reason } => reason.clone(),
        StalenessSignal::Modified { files } => format!("{} edited", summarize_files(files)),
        StalenessSignal::Orphan { files } => format!("{} untracked", summarize_files(files)),
    }
}

fn summarize_files(files: &[PathBuf]) -> String {
    if files.is_empty() {
        return "unknown file".to_string();
    }

    let mut names: Vec<String> = files
        .iter()
        .take(2)
        .map(|path| path.display().to_string())
        .collect();
    if files.len() > names.len() {
        names.push(format!("+{} more", files.len() - names.len()));
    }
    names.join(", ")
}
