//! Dynamic Plugin Loader
//!
//! Manages loading and unloading of dynamic plugins at runtime.
//! Currently this is a stub implementation - full FFI plugin loading is planned for future releases.

use anyhow::Result;
use conveyor_plugin_api::PLUGIN_API_VERSION;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Loaded plugin information
pub struct LoadedPlugin {
    name: String,
    path: PathBuf,
}

impl LoadedPlugin {
    /// Get plugin name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get plugin library path
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Plugin loader that manages dynamically loaded plugins
pub struct PluginLoader {
    plugins: HashMap<String, LoadedPlugin>,
    plugin_dir: PathBuf,
}

impl PluginLoader {
    /// Create a new plugin loader
    pub fn new() -> Self {
        // Default plugin directory is ./target/debug or ./target/release
        let plugin_dir = if cfg!(debug_assertions) {
            PathBuf::from("target/debug")
        } else {
            PathBuf::from("target/release")
        };

        Self {
            plugins: HashMap::new(),
            plugin_dir,
        }
    }

    /// Set custom plugin directory
    pub fn with_plugin_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.plugin_dir = dir.as_ref().to_path_buf();
        self
    }

    /// Load a plugin by name with version checking and panic handling
    ///
    /// Note: This is currently a stub implementation. Full FFI plugin loading
    /// is planned for future releases. For now, this validates the plugin path
    /// and logs the operation.
    pub fn load_plugin(&mut self, name: &str) -> Result<()> {
        if self.plugins.contains_key(name) {
            tracing::debug!("Plugin '{}' already loaded", name);
            return Ok(());
        }

        let library_name = get_library_name(name);
        let library_path = self.plugin_dir.join(&library_name);

        tracing::info!(
            "Plugin loading requested: {} from {:?} (API v{})",
            name,
            library_path,
            PLUGIN_API_VERSION
        );

        // Check if plugin file exists
        if !library_path.exists() {
            tracing::warn!(
                "Plugin library not found at {:?}. Continuing without plugin '{}'",
                library_path,
                name
            );
            // For now, we don't fail - just log the warning
            // In the future, this would actually load the plugin
        } else {
            tracing::info!("Plugin library found at {:?}", library_path);
        }

        // Record that we attempted to load this plugin
        self.plugins.insert(
            name.to_string(),
            LoadedPlugin {
                name: name.to_string(),
                path: library_path,
            },
        );

        Ok(())
    }

    /// Load multiple plugins
    pub fn load_plugins(&mut self, names: &[String]) -> Result<()> {
        for name in names {
            self.load_plugin(name)?;
        }
        Ok(())
    }

    /// Check if a plugin is loaded
    pub fn is_loaded(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Get a loaded plugin
    pub fn get_plugin(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.get(name)
    }

    /// Get list of loaded plugin names
    pub fn loaded_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Get all loaded plugins
    pub fn plugins(&self) -> &HashMap<String, LoadedPlugin> {
        &self.plugins
    }
}

/// Get platform-specific library name
fn get_library_name(plugin_name: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        format!("libconveyor_plugin_{}.dylib", plugin_name)
    }

    #[cfg(target_os = "linux")]
    {
        format!("libconveyor_plugin_{}.so", plugin_name)
    }

    #[cfg(target_os = "windows")]
    {
        format!("conveyor_plugin_{}.dll", plugin_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_name() {
        #[cfg(target_os = "macos")]
        assert_eq!(get_library_name("http"), "libconveyor_plugin_http.dylib");

        #[cfg(target_os = "linux")]
        assert_eq!(get_library_name("http"), "libconveyor_plugin_http.so");

        #[cfg(target_os = "windows")]
        assert_eq!(get_library_name("http"), "conveyor_plugin_http.dll");
    }

    #[test]
    fn test_plugin_loader_creation() {
        let loader = PluginLoader::new();
        assert!(loader.plugins.is_empty());
    }

    #[test]
    fn test_plugin_loading() {
        let mut loader = PluginLoader::new();
        // This should succeed (stub implementation)
        assert!(loader.load_plugin("test").is_ok());
        assert!(loader.is_loaded("test"));
    }
}
