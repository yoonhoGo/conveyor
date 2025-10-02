//! Dynamic Plugin Loader (Version 2)
//!
//! Manages loading and unloading of dynamic plugins at runtime using FFI.
//!
//! Version 2 supports unified FfiStage interface with input-aware execution.

use anyhow::{anyhow, Context, Result};
use conveyor_plugin_api::traits::{FfiStage_TO, StageFactory};
use conveyor_plugin_api::{PluginCapability, PluginDeclaration, RBox, PLUGIN_API_VERSION};
use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Loaded plugin information
pub struct LoadedPlugin {
    name: String,
    version: String,
    description: String,
    capabilities: Vec<PluginCapability>,
    library: Library, // Keep library alive and accessible for factory loading
}

impl LoadedPlugin {
    /// Get plugin name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get plugin version
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get plugin description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get plugin capabilities
    pub fn capabilities(&self) -> &[PluginCapability] {
        &self.capabilities
    }

    /// Create a stage instance by name
    ///
    /// This loads the factory function from the plugin library and calls it
    /// to create a new stage instance.
    pub fn create_stage(&self, stage_name: &str) -> Result<FfiStage_TO<'static, RBox<()>>> {
        // Find the capability for this stage
        let capability = self
            .capabilities
            .iter()
            .find(|c| c.name.as_str() == stage_name)
            .ok_or_else(|| anyhow!("Stage '{}' not found in plugin '{}'", stage_name, self.name))?;

        // Load the factory function from the library
        unsafe {
            // Add null terminator to symbol name
            let mut symbol_name = capability.factory_symbol.as_str().to_string();
            symbol_name.push('\0');

            let factory: Symbol<StageFactory> =
                self.library.get(symbol_name.as_bytes()).with_context(|| {
                    format!(
                        "Failed to load factory symbol '{}' for stage '{}'",
                        capability.factory_symbol, stage_name
                    )
                })?;

            // Call the factory to create a new stage instance
            Ok(factory())
        }
    }

    /// List available stages
    pub fn stage_names(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .map(|c| c.name.to_string())
            .collect()
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
    pub fn load_plugin(&mut self, name: &str) -> Result<()> {
        if self.plugins.contains_key(name) {
            tracing::debug!("Plugin '{}' already loaded", name);
            return Ok(());
        }

        let library_name = get_library_name(name);
        let library_path = self.plugin_dir.join(&library_name);

        tracing::info!(
            "Loading plugin: {} from {:?} (API v{})",
            name,
            library_path,
            PLUGIN_API_VERSION
        );

        // Check if plugin file exists
        if !library_path.exists() {
            return Err(anyhow!("Plugin library not found at {:?}", library_path));
        }

        // Load the library with panic isolation
        let (library, declaration) = self.load_library_safe(&library_path, name)?;

        // Verify API compatibility
        if !declaration.is_compatible() {
            return Err(anyhow!(
                "Plugin '{}' API version {} is incompatible with host API version {}",
                name,
                declaration.api_version,
                PLUGIN_API_VERSION
            ));
        }

        tracing::info!(
            "Plugin '{}' v{} loaded successfully: {}",
            declaration.name,
            declaration.version,
            declaration.description
        );

        // Get plugin capabilities
        let capabilities = (declaration.get_capabilities)();
        let capabilities: Vec<PluginCapability> = capabilities.into();

        if capabilities.is_empty() {
            return Err(anyhow!("Plugin '{}' provides no stages", name));
        }

        tracing::info!(
            "Plugin '{}' provides {} stage(s): {}",
            name,
            capabilities.len(),
            capabilities
                .iter()
                .map(|c| format!("{} ({})", c.name, c.stage_type.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Store the loaded plugin
        self.plugins.insert(
            name.to_string(),
            LoadedPlugin {
                name: declaration.name.to_string(),
                version: declaration.version.to_string(),
                description: declaration.description.to_string(),
                capabilities,
                library,
            },
        );

        Ok(())
    }

    /// Load library with panic isolation
    fn load_library_safe(
        &self,
        library_path: &Path,
        name: &str,
    ) -> Result<(Library, &'static PluginDeclaration)> {
        // Catch panics during loading
        let result = std::panic::catch_unwind(|| self.load_library_internal(library_path));

        match result {
            Ok(Ok(lib_and_decl)) => Ok(lib_and_decl),
            Ok(Err(e)) => Err(e),
            Err(panic_err) => {
                let panic_msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };

                Err(anyhow!(
                    "Plugin '{}' panicked during loading: {}",
                    name,
                    panic_msg
                ))
            }
        }
    }

    /// Internal library loading (can panic)
    fn load_library_internal(
        &self,
        library_path: &Path,
    ) -> Result<(Library, &'static PluginDeclaration)> {
        unsafe {
            // Load the library
            let library = Library::new(library_path)
                .with_context(|| format!("Failed to load library from {:?}", library_path))?;

            // Get the plugin declaration symbol
            let declaration: libloading::Symbol<*const PluginDeclaration> = library
                .get(b"_plugin_declaration\0")
                .context("Plugin does not export '_plugin_declaration' symbol")?;

            let declaration_ref = &**declaration;

            Ok((library, declaration_ref))
        }
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

    /// Create a stage from any loaded plugin
    ///
    /// Searches all loaded plugins for a stage with the given name.
    pub fn create_stage(&self, stage_name: &str) -> Result<FfiStage_TO<'static, RBox<()>>> {
        for plugin in self.plugins.values() {
            if plugin
                .capabilities
                .iter()
                .any(|c| c.name.as_str() == stage_name)
            {
                return plugin.create_stage(stage_name);
            }
        }

        Err(anyhow!(
            "Stage '{}' not found in any loaded plugin",
            stage_name
        ))
    }

    /// Find which plugin provides a stage
    pub fn find_plugin_for_stage(&self, stage_name: &str) -> Option<&LoadedPlugin> {
        self.plugins.values().find(|plugin| {
            plugin
                .capabilities
                .iter()
                .any(|c| c.name.as_str() == stage_name)
        })
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
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
    fn test_plugin_loading_not_found() {
        let mut loader = PluginLoader::new();
        // This should fail - plugin doesn't exist
        let result = loader.load_plugin("nonexistent");
        assert!(result.is_err());
        assert!(!loader.is_loaded("nonexistent"));
    }

    #[test]
    fn test_duplicate_loading() {
        let mut loader = PluginLoader::new();
        // First load will fail (no plugin file)
        let _ = loader.load_plugin("test");

        // If plugin was somehow loaded, second load should be ok (already loaded)
        if loader.is_loaded("test") {
            assert!(loader.load_plugin("test").is_ok());
        }
    }
}
