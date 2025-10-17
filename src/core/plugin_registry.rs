use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Registry metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub version: String,
    pub registry_url: String,
    pub plugins: HashMap<String, PluginMetadata>,
}

/// Plugin metadata in registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub repository: String,
    pub downloads: HashMap<String, DownloadInfo>,
}

/// Download information for a specific platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub url: String,
    pub checksum: String,
}

/// Installed plugin version info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

impl Registry {
    /// Fetch registry from remote URL
    pub async fn fetch_remote(url: &str) -> Result<Self> {
        let response = reqwest::get(url)
            .await
            .context("Failed to fetch registry from remote")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch registry: HTTP {}",
                response.status()
            ));
        }

        let registry: Registry = response
            .json()
            .await
            .context("Failed to parse registry JSON")?;

        Ok(registry)
    }

    /// Load registry from local cache
    pub fn load_cached(cache_path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(cache_path)
            .context("Failed to read cached registry")?;

        let registry: Registry =
            serde_json::from_str(&content).context("Failed to parse cached registry")?;

        Ok(registry)
    }

    /// Save registry to local cache
    pub fn save_cache(&self, cache_path: &PathBuf) -> Result<()> {
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }

        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize registry")?;

        std::fs::write(cache_path, content).context("Failed to write registry cache")?;

        Ok(())
    }

    /// Get plugin metadata by name
    pub fn get_plugin(&self, name: &str) -> Option<&PluginMetadata> {
        self.plugins.get(name)
    }

    /// List all available plugins
    pub fn list_plugins(&self) -> Vec<&PluginMetadata> {
        self.plugins.values().collect()
    }
}

impl PluginMetadata {
    /// Get download info for current platform
    pub fn get_download_info(&self) -> Option<&DownloadInfo> {
        let platform = detect_platform();
        self.downloads.get(&platform)
    }
}

/// Detect current platform
pub fn detect_platform() -> String {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "darwin-aarch64".to_string()
    }

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "darwin-x86_64".to_string()
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Fallback (shouldn't reach here for macOS-only builds)
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_platform() {
        let platform = detect_platform();
        #[cfg(target_os = "macos")]
        assert!(platform.starts_with("darwin-"));
    }

    #[test]
    fn test_parse_registry() {
        let json = r#"{
            "version": "1.0",
            "registry_url": "https://example.com/registry.json",
            "plugins": {
                "test": {
                    "name": "test",
                    "version": "0.1.0",
                    "description": "Test plugin",
                    "author": "Test Author",
                    "repository": "https://github.com/test/test",
                    "downloads": {
                        "darwin-aarch64": {
                            "url": "https://example.com/plugin.dylib",
                            "checksum": "abc123"
                        }
                    }
                }
            }
        }"#;

        let registry: Registry = serde_json::from_str(json).unwrap();
        assert_eq!(registry.version, "1.0");
        assert_eq!(registry.plugins.len(), 1);

        let plugin = registry.get_plugin("test").unwrap();
        assert_eq!(plugin.name, "test");
        assert_eq!(plugin.version, "0.1.0");
    }
}
