use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tracing::{debug, info};

const GITHUB_REPO: &str = "yoonhoGo/conveyor";
const CHECK_INTERVAL_SECS: u64 = 86400; // 24 hours
const VERSION_CHECK_FILE: &str = ".conveyor_version_check";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: String,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Check if a new version is available on GitHub
pub async fn check_for_updates(silent: bool) -> Result<Option<String>> {
    // Skip if checked recently
    if !silent && !should_check_version()? {
        return Ok(None);
    }

    let current_version = env!("CARGO_PKG_VERSION");
    debug!("Current version: {}", current_version);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let response = match client
        .get(&url)
        .header("User-Agent", "conveyor-cli")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            debug!("Failed to check for updates: {}", e);
            return Ok(None);
        }
    };

    if !response.status().is_success() {
        debug!("GitHub API returned status: {}", response.status());
        return Ok(None);
    }

    let release: GithubRelease = response.json().await?;

    // Skip prereleases
    if release.prerelease {
        return Ok(None);
    }

    let latest_version = release.tag_name.trim_start_matches('v');

    // Update last check time
    update_version_check_time()?;

    if is_newer_version(current_version, latest_version) {
        if !silent {
            println!(
                "\n🎉 New version available: {} (current: {})",
                latest_version, current_version
            );
            println!("Run 'conveyor update' to install the latest version\n");
        }
        Ok(Some(latest_version.to_string()))
    } else {
        debug!("Already on latest version");
        Ok(None)
    }
}

/// Install the latest version from GitHub releases
pub async fn install_update() -> Result<()> {
    info!("Checking for updates...");

    let current_version = env!("CARGO_PKG_VERSION");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let response = client
        .get(&url)
        .header("User-Agent", "conveyor-cli")
        .send()
        .await
        .context("Failed to fetch release information")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch releases: HTTP {}",
            response.status()
        ));
    }

    let release: GithubRelease = response.json().await?;
    let latest_version = release.tag_name.trim_start_matches('v');

    if !is_newer_version(current_version, latest_version) {
        println!("✓ Already on the latest version ({})", current_version);
        return Ok(());
    }

    println!("Updating from {} to {}...", current_version, latest_version);

    // Determine platform-specific binary name
    let (os, arch) = get_platform_info()?;
    let binary_name = format!("conveyor-{}-{}", os, arch);

    // Find matching asset
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(&binary_name))
        .ok_or_else(|| anyhow!("No binary found for platform: {}-{}", os, arch))?;

    println!("Downloading {}...", asset.name);

    // Download binary
    let binary_data = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .bytes()
        .await?;

    // Get current executable path
    let current_exe = env::current_exe()?;
    let backup_path = current_exe.with_extension("backup");

    // Backup current binary
    fs::copy(&current_exe, &backup_path).context("Failed to create backup of current binary")?;

    // Write new binary
    fs::write(&current_exe, &binary_data).context("Failed to write new binary")?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&current_exe, perms)?;
    }

    println!("✓ Successfully updated to version {}", latest_version);
    println!("Backup saved to: {:?}", backup_path);

    Ok(())
}

/// Check if we should perform a version check based on last check time
fn should_check_version() -> Result<bool> {
    let check_file = get_version_check_file()?;

    if !check_file.exists() {
        return Ok(true);
    }

    let metadata = fs::metadata(&check_file)?;
    let modified = metadata.modified()?;
    let elapsed = SystemTime::now().duration_since(modified)?;

    Ok(elapsed.as_secs() > CHECK_INTERVAL_SECS)
}

/// Update the version check timestamp
fn update_version_check_time() -> Result<()> {
    let check_file = get_version_check_file()?;
    fs::write(&check_file, "")?;
    Ok(())
}

/// Get the path to the version check file
fn get_version_check_file() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    Ok(home.join(VERSION_CHECK_FILE))
}

/// Compare two semantic versions
fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ))
    };

    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Get platform information for binary selection (macOS only)
fn get_platform_info() -> Result<(&'static str, &'static str)> {
    if env::consts::OS != "macos" {
        return Err(anyhow!("This binary only supports macOS"));
    }

    let os = "darwin";

    let arch = match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => return Err(anyhow!("Unsupported architecture: {}", other)),
    };

    Ok((os, arch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0", "1.0.0"));
        assert!(is_newer_version("1.2.3", "1.2.4"));
        assert!(is_newer_version("1.2.3", "1.3.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("2.0.0", "1.0.0"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_platform_info() {
        let (os, arch) = get_platform_info().unwrap();
        assert_eq!(os, "darwin");
        assert!(arch == "x64" || arch == "arm64");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_platform_info_unsupported() {
        let result = get_platform_info();
        assert!(result.is_err());
    }
}
