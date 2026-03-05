//! `orchestra update` — check GitHub for a newer release and advise the user.

use anyhow::Result;
use colored::Colorize;
use serde::Deserialize;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = "chris-miracle/orch";

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
}

pub fn run() -> Result<()> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let response = ureq::get(&url)
        .set("User-Agent", &format!("orchestra/{CURRENT_VERSION}"))
        .call();

    match response {
        Ok(resp) => {
            let release: Release = resp.into_json()?;
            let latest = release.tag_name.trim_start_matches('v');

            match version_cmp(CURRENT_VERSION, latest) {
                std::cmp::Ordering::Less => {
                    println!(
                        "\n  {} A new version of Orchestra is available: {} → {}\n",
                        "→".cyan().bold(),
                        format!("v{CURRENT_VERSION}").dimmed(),
                        format!("v{latest}").green().bold(),
                    );
                    println!("  To upgrade:\n");
                    println!("    {}", "brew upgrade orchestra".yellow().bold());
                    println!("      or");
                    println!(
                        "    {}",
                        format!(
                            "curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | sh"
                        )
                        .yellow()
                        .bold()
                    );
                    println!();
                }
                std::cmp::Ordering::Equal => {
                    println!(
                        "\n  {} You're on the latest version: {}\n",
                        "✓".green().bold(),
                        format!("v{CURRENT_VERSION}").green()
                    );
                }
                std::cmp::Ordering::Greater => {
                    println!(
                        "\n  {} You're on a pre-release version: {} (latest stable: {})\n",
                        "→".cyan().bold(),
                        format!("v{CURRENT_VERSION}").cyan(),
                        format!("v{latest}").dimmed()
                    );
                }
            }
        }
        Err(ureq::Error::Status(code, _)) => {
            eprintln!(
                "{} Could not reach GitHub releases (HTTP {code}). Check your connection.",
                "✗".red().bold()
            );
        }
        Err(e) => {
            eprintln!(
                "{} Could not reach GitHub releases: {e}",
                "✗".red().bold()
            );
        }
    }

    Ok(())
}

/// Compare two semver-ish strings ("1.2.3") without pulling in semver crate.
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| {
        s.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let va = parse(a);
    let vb = parse(b);
    va.cmp(&vb)
}
