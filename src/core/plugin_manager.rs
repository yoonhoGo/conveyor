use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};

use super::plugin_registry::{InstalledPlugin, Registry};

const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/yoonhoGo/conveyor/main/registry.json";

/// Plugin manager for downloading, installing, and managing plugins
pub struct PluginManager {
    registry_cache_path: PathBuf,
    plugins_dir: PathBuf,
    versions_file: PathBuf,
}

/// Installed plugins version tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct VersionsFile {
    plugins: HashMap<String, String>,
}

impl PluginManager {
    /// Create new plugin manager with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        let conveyor_dir = home_dir.join(".conveyor");
        let plugins_dir = conveyor_dir.join("plugins");
        let registry_cache_path = conveyor_dir.join("registry.json");
        let versions_file = plugins_dir.join("versions.json");

        Ok(Self {
            registry_cache_path,
            plugins_dir,
            versions_file,
        })
    }

    /// Get or update registry
    pub async fn get_registry(&self, force_update: bool) -> Result<Registry> {
        if force_update || !self.registry_cache_path.exists() {
            info!("Fetching registry from remote...");
            let registry = Registry::fetch_remote(DEFAULT_REGISTRY_URL).await?;
            registry.save_cache(&self.registry_cache_path)?;
            Ok(registry)
        } else {
            Registry::load_cached(&self.registry_cache_path)
        }
    }

    /// Install a plugin by name
    pub async fn install_plugin(&self, name: &str) -> Result<()> {
        info!("Installing plugin '{}'...", name);

        // Get registry
        let registry = self.get_registry(false).await?;

        // Find plugin metadata
        let plugin_meta = registry
            .get_plugin(name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found in registry", name))?;

        // Get download info for current platform
        let download_info = plugin_meta
            .get_download_info()
            .ok_or_else(|| anyhow!("Plugin '{}' not available for current platform", name))?;

        // Download plugin
        info!("Downloading from {}...", download_info.url);
        let response = reqwest::get(&download_info.url)
            .await
            .context("Failed to download plugin")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download plugin: HTTP {}",
                response.status()
            ));
        }

        let plugin_data = response
            .bytes()
            .await
            .context("Failed to read plugin data")?;

        // Verify checksum if provided
        if !download_info.checksum.is_empty() {
            let computed_checksum = compute_sha256(&plugin_data);
            let expected_checksum = download_info.checksum.replace("sha256:", "");

            if computed_checksum != expected_checksum {
                return Err(anyhow!(
                    "Checksum verification failed for plugin '{}'",
                    name
                ));
            }
            info!("✓ Checksum verified");
        } else {
            warn!("No checksum provided, skipping verification");
        }

        // Create plugins directory if it doesn't exist
        std::fs::create_dir_all(&self.plugins_dir).context("Failed to create plugins directory")?;

        // Determine plugin filename
        let filename = format!("libconveyor_plugin_{}.dylib", name);
        let plugin_path = self.plugins_dir.join(&filename);

        // Write plugin file
        std::fs::write(&plugin_path, &plugin_data).context("Failed to write plugin file")?;

        info!("✓ Plugin installed to {}", plugin_path.display());

        // Update versions file
        self.update_versions_file(name, &plugin_meta.version)?;

        Ok(())
    }

    /// List installed plugins
    pub fn list_installed(&self) -> Result<Vec<InstalledPlugin>> {
        if !self.versions_file.exists() {
            return Ok(Vec::new());
        }

        let versions = self.load_versions_file()?;
        let mut installed = Vec::new();

        for (name, version) in versions.plugins.iter() {
            let filename = format!("libconveyor_plugin_{}.dylib", name);
            let path = self.plugins_dir.join(&filename);

            if path.exists() {
                installed.push(InstalledPlugin {
                    name: name.clone(),
                    version: version.clone(),
                    path,
                });
            }
        }

        Ok(installed)
    }

    /// Check if a plugin is installed
    pub fn is_installed(&self, name: &str) -> bool {
        let filename = format!("libconveyor_plugin_{}.dylib", name);
        let path = self.plugins_dir.join(&filename);
        path.exists()
    }

    /// Get plugins directory path
    pub fn plugins_dir(&self) -> &PathBuf {
        &self.plugins_dir
    }

    /// Uninstall a plugin
    pub fn uninstall_plugin(&self, name: &str) -> Result<()> {
        let filename = format!("libconveyor_plugin_{}.dylib", name);
        let path = self.plugins_dir.join(&filename);

        if !path.exists() {
            return Err(anyhow!("Plugin '{}' is not installed", name));
        }

        std::fs::remove_file(&path).context("Failed to remove plugin file")?;

        // Update versions file
        let mut versions = self.load_versions_file()?;
        versions.plugins.remove(name);
        self.save_versions_file(&versions)?;

        info!("✓ Plugin '{}' uninstalled", name);
        Ok(())
    }

    // Helper: Load versions file
    fn load_versions_file(&self) -> Result<VersionsFile> {
        if !self.versions_file.exists() {
            return Ok(VersionsFile::default());
        }

        let content =
            std::fs::read_to_string(&self.versions_file).context("Failed to read versions file")?;

        let versions: VersionsFile =
            serde_json::from_str(&content).context("Failed to parse versions file")?;

        Ok(versions)
    }

    // Helper: Save versions file
    fn save_versions_file(&self, versions: &VersionsFile) -> Result<()> {
        let content =
            serde_json::to_string_pretty(versions).context("Failed to serialize versions")?;

        std::fs::write(&self.versions_file, content).context("Failed to write versions file")?;

        Ok(())
    }

    // Helper: Update versions file with new plugin
    fn update_versions_file(&self, name: &str, version: &str) -> Result<()> {
        let mut versions = self.load_versions_file()?;
        versions
            .plugins
            .insert(name.to_string(), version.to_string());
        self.save_versions_file(&versions)?;
        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new().expect("Failed to create plugin manager")
    }
}

/// Compute SHA256 checksum of data
fn compute_sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sha256() {
        let data = b"hello world";
        let checksum = compute_sha256(data);
        assert_eq!(
            checksum,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
