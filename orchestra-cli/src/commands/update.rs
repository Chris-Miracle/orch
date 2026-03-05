//! `orchestra update` — auto-upgrade Orchestra to the latest release.
//!
//! Channel is baked in at compile-time by CI (`ORCHESTRA_RELEASE_CHANNEL`),
//! persisted in `~/.orchestra/channel`, and overridable with --stable / --beta.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;
use serde::Deserialize;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = "Chris-Miracle/orch";

/// Release channel baked in by CI. Defaults to "stable" for local/dev builds.
const COMPILED_CHANNEL: Option<&str> = option_env!("ORCHESTRA_RELEASE_CHANNEL");

/// Exact release tag baked in by CI (e.g. "v0.1.8" or "v0.1.8-beta.42").
/// Falls back to "v{CURRENT_VERSION}" for local/dev builds.
const COMPILED_TAG: Option<&str> = option_env!("ORCHESTRA_RELEASE_TAG");

/// Asset filename for this binary's architecture.
#[cfg(target_arch = "aarch64")]
const ASSET_NAME: &str = "orchestra-macos-arm64.tar.gz";
#[cfg(target_arch = "x86_64")]
const ASSET_NAME: &str = "orchestra-macos-x86_64.tar.gz";
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
compile_error!("Unsupported architecture — Orchestra only supports aarch64 and x86_64 on macOS.");

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

/// Check for and apply the latest Orchestra update.
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Switch to the stable release channel and update.
    #[arg(long, conflicts_with = "beta")]
    pub stable: bool,

    /// Switch to the beta pre-release channel and update.
    #[arg(long, conflicts_with = "stable")]
    pub beta: bool,
}

// ---------------------------------------------------------------------------
// GitHub API types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(args: UpdateArgs) -> Result<()> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let channel_file = home.join(".orchestra").join("channel");

    // 1. Resolve effective channel
    let channel: String = if args.stable {
        "stable".into()
    } else if args.beta {
        "beta".into()
    } else {
        read_channel_from_disk(&channel_file)
    };

    // 2. Persist channel override
    if args.stable || args.beta {
        save_channel(&channel_file, &channel)?;
        println!(
            "\n  {} Switched to {} channel.",
            "✓".green().bold(),
            channel.bold()
        );
    }

    // 3. Current installed tag
    let installed_tag = COMPILED_TAG
        .map(str::to_string)
        .unwrap_or_else(|| format!("v{CURRENT_VERSION}"));

    println!("\n  channel     {}", channel.bold());
    println!("  installed   {}", installed_tag.dimmed());

    // 4. Fetch latest release for channel
    let release =
        fetch_release(&channel).context("failed to fetch release info from GitHub")?;
    let latest_tag = &release.tag_name;

    // 5. Already up to date?
    if latest_tag == &installed_tag {
        println!(
            "\n  {} Already on the latest {} release: {}\n",
            "✓".green().bold(),
            channel,
            latest_tag.green()
        );
        return Ok(());
    }

    println!(
        "\n  {} Update available: {} → {}\n",
        "→".cyan().bold(),
        installed_tag.dimmed(),
        latest_tag.green().bold()
    );

    // 6. Find downloadable asset for this architecture
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == ASSET_NAME)
        .with_context(|| {
            format!(
                "no asset '{ASSET_NAME}' in release {latest_tag} — \
                 the release may still be building, try again in a minute"
            )
        })?;

    // 7. Determine install path (where *this* binary lives)
    let install_path = std::env::current_exe()
        .context("could not determine current executable path")?
        .canonicalize()
        .context("could not resolve executable path")?;

    // 8. Download, extract, and replace
    println!("  Downloading {}...", asset.name);
    download_and_install(&asset.browser_download_url, &install_path)
        .context("update failed")?;

    // 9. Persist updated channel tag (so next `orchestra update` knows what's installed)
    let tag_file = home.join(".orchestra").join("installed_tag");
    let _ = fs::write(&tag_file, latest_tag);

    println!(
        "\n  {} Updated: {} → {}\n",
        "✓".green().bold(),
        installed_tag.dimmed(),
        latest_tag.green().bold()
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Channel helpers
// ---------------------------------------------------------------------------

fn read_channel_from_disk(channel_file: &Path) -> String {
    if let Ok(s) = fs::read_to_string(channel_file) {
        let s = s.trim().to_string();
        if s == "beta" || s == "stable" {
            return s;
        }
    }
    COMPILED_CHANNEL.unwrap_or("stable").to_string()
}

fn save_channel(channel_file: &Path, channel: &str) -> Result<()> {
    let dir = channel_file.parent().expect("channel file has parent dir");
    fs::create_dir_all(dir).context("could not create ~/.orchestra directory")?;
    fs::write(channel_file, channel).context("could not write ~/.orchestra/channel")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// GitHub API
// ---------------------------------------------------------------------------

fn fetch_release(channel: &str) -> Result<Release> {
    if channel == "beta" {
        fetch_latest_prerelease()
    } else {
        fetch_latest_stable()
    }
}

fn fetch_latest_stable() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    ureq::get(&url)
        .set("User-Agent", &format!("orchestra/{CURRENT_VERSION}"))
        .call()
        .context("GitHub API unreachable — check your connection")?
        .into_json::<Release>()
        .context("unexpected response from GitHub releases API")
}

fn fetch_latest_prerelease() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=20");
    let releases: Vec<Release> = ureq::get(&url)
        .set("User-Agent", &format!("orchestra/{CURRENT_VERSION}"))
        .call()
        .context("GitHub API unreachable — check your connection")?
        .into_json()
        .context("unexpected response from GitHub releases API")?;

    releases
        .into_iter()
        .find(|r| r.prerelease)
        .context(
            "no pre-release found — there may not be any beta releases yet. \
             Try: orchestra update --stable",
        )
}

// ---------------------------------------------------------------------------
// Download + install
// ---------------------------------------------------------------------------

fn download_and_install(url: &str, install_path: &Path) -> Result<()> {
    // Temp working directory
    let tmp_dir: PathBuf = std::env::temp_dir()
        .join(format!("orchestra-update-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).context("could not create temp directory")?;

    let archive_path = tmp_dir.join(ASSET_NAME);
    let extracted_bin = tmp_dir.join("orchestra");
    let staging_path = install_path.with_file_name("orchestra_update_staging");

    // Guard: clean up temp dir on exit
    let _cleanup = defer_cleanup(tmp_dir.clone());

    // Download
    let response = ureq::get(url)
        .set("User-Agent", &format!("orchestra/{CURRENT_VERSION}"))
        .call()
        .context("download request failed")?;

    let mut archive_file =
        fs::File::create(&archive_path).context("could not create temp archive file")?;
    let mut reader = response.into_reader();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf).context("download interrupted")?;
        if n == 0 {
            break;
        }
        archive_file.write_all(&buf[..n]).context("write error")?;
    }
    drop(archive_file);

    // Extract
    let status = Command::new("tar")
        .args([
            "-xzf",
            archive_path.to_str().unwrap(),
            "-C",
            tmp_dir.to_str().unwrap(),
        ])
        .status()
        .context("failed to run tar")?;
    if !status.success() {
        bail!("tar extraction failed (exit {})", status);
    }

    if !extracted_bin.exists() {
        bail!("extracted archive did not contain an 'orchestra' binary");
    }

    // Stage next to install path (same filesystem → rename is atomic)
    fs::copy(&extracted_bin, &staging_path)
        .context("failed to stage updated binary")?;

    // Set executable bit
    Command::new("chmod")
        .args(["+x", staging_path.to_str().unwrap()])
        .status()
        .context("chmod failed")?;

    // Atomic replace
    fs::rename(&staging_path, install_path).with_context(|| {
        format!(
            "could not replace '{}' — try running with sudo if the binary is in a system directory",
            install_path.display()
        )
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Minimal defer helper (no dep needed)
// ---------------------------------------------------------------------------

struct DeferCleanup(PathBuf);
impl Drop for DeferCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn defer_cleanup(path: PathBuf) -> DeferCleanup {
    DeferCleanup(path)
}

