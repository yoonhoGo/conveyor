//! WASM Plugin Loader (Version 2)
//!
//! Loads and manages WebAssembly plugins using Wasmtime Component Model.
//!
//! Version 2 supports unified execute interface with input-aware execution.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

// For home directory access

// Generate host-side bindings from WIT file
wasmtime::component::bindgen!({
    path: "conveyor-wasm-plugin-api/wit/conveyor-plugin.wit",
    world: "plugin",
    async: true,
});

/// Host state for WASM plugin execution
struct PluginState {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

/// WASM plugin handle
pub struct WasmPluginHandle {
    component: Component,
    metadata: PluginMetadata,
    capabilities: Vec<StageCapability>,
}

impl WasmPluginHandle {
    /// Get plugin name
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Get plugin version
    pub fn version(&self) -> &str {
        &self.metadata.version
    }

    /// Get plugin description
    pub fn description(&self) -> &str {
        &self.metadata.description
    }

    /// Get plugin capabilities
    pub fn capabilities(&self) -> &[StageCapability] {
        &self.capabilities
    }

    /// List available stages
    pub fn stage_names(&self) -> Vec<String> {
        self.capabilities.iter().map(|c| c.name.clone()).collect()
    }

    /// Check if plugin provides a stage
    pub fn has_stage(&self, stage_name: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == stage_name)
    }
}

/// WASM Plugin Loader
pub struct WasmPluginLoader {
    engine: Engine,
    plugin_dir: PathBuf,
    plugins: HashMap<String, WasmPluginHandle>,
}

impl WasmPluginLoader {
    /// Create a new WASM plugin loader
    pub fn new() -> Result<Self> {
        // Configure Wasmtime engine
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        // Default plugin directory: target/wasm32-wasip2/release
        let plugin_dir = PathBuf::from("target/wasm32-wasip2/release");

        Ok(Self {
            engine,
            plugin_dir,
            plugins: HashMap::new(),
        })
    }

    /// Set custom plugin directory
    pub fn with_plugin_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.plugin_dir = dir.into();
        self
    }

    /// Load a WASM plugin by name
    pub async fn load_plugin(&mut self, name: &str) -> Result<()> {
        let plugin_filename = format!("conveyor_plugin_{}.wasm", name);

        // Search for plugin in multiple locations
        let search_paths = get_wasm_plugin_search_paths();
        let mut plugin_path = None;

        for search_dir in search_paths {
            let candidate = search_dir.join(&plugin_filename);
            if candidate.exists() {
                plugin_path = Some(candidate);
                break;
            }
        }

        let plugin_path = plugin_path.ok_or_else(|| {
            anyhow::anyhow!(
                "WASM plugin '{}' not found. Searched:\n  - ~/.conveyor/wasm-plugins/\n  - target/wasm32-wasip2/release/",
                name
            )
        })?;

        tracing::info!("Loading WASM plugin '{}' from {:?}", name, plugin_path);

        // Load component
        let component = Component::from_file(&self.engine, &plugin_path)
            .with_context(|| format!("Failed to load WASM component from {:?}", plugin_path))?;

        // Create store with WASI context and file system access
        let current_dir = std::env::current_dir()?;
        let current_dir_str = current_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Current directory path contains invalid UTF-8"))?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .preopened_dir(
                current_dir_str,
                ".",
                wasmtime_wasi::DirPerms::all(),
                wasmtime_wasi::FilePerms::all(),
            )?
            .build();
        let table = ResourceTable::new();
        let state = PluginState { wasi, table };
        let mut store = Store::new(&self.engine, state);

        // Create linker and add WASI
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Instantiate component
        let plugin = Plugin::instantiate_async(&mut store, &component, &linker)
            .await
            .with_context(|| format!("Failed to instantiate WASM plugin '{}'", name))?;

        // Get metadata
        let metadata = plugin
            .call_get_metadata(&mut store)
            .await
            .with_context(|| format!("Failed to get metadata from WASM plugin '{}'", name))?;

        // Get capabilities
        let capabilities = plugin
            .call_get_capabilities(&mut store)
            .await
            .with_context(|| format!("Failed to get capabilities from WASM plugin '{}'", name))?;

        if capabilities.is_empty() {
            anyhow::bail!("WASM plugin '{}' provides no stages", name);
        }

        tracing::info!(
            "Loaded WASM plugin: {} v{} (API v{}) with {} stage(s): {}",
            metadata.name,
            metadata.version,
            metadata.api_version,
            capabilities.len(),
            capabilities
                .iter()
                .map(|c| format!("{} ({:?})", c.name, c.stage_type))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Store plugin handle (use metadata name as key for consistency)
        let plugin_name = metadata.name.clone();
        let handle = WasmPluginHandle {
            component,
            metadata,
            capabilities,
        };

        self.plugins.insert(plugin_name, handle);

        Ok(())
    }

    /// Load multiple plugins
    pub async fn load_plugins(&mut self, names: &[String]) -> Result<()> {
        for name in names {
            if let Err(e) = self.load_plugin(name).await {
                tracing::warn!("Failed to load WASM plugin '{}': {}", name, e);
            }
        }
        Ok(())
    }

    /// Get loaded plugin
    pub fn get_plugin(&self, name: &str) -> Option<&WasmPluginHandle> {
        self.plugins.get(name)
    }

    /// Get loaded plugin metadata
    pub fn get_plugin_metadata(&self, name: &str) -> Option<&PluginMetadata> {
        self.plugins.get(name).map(|h| &h.metadata)
    }

    /// List all loaded plugins
    pub fn list_plugins(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    /// Find which plugin provides a stage
    pub fn find_plugin_for_stage(&self, stage_name: &str) -> Option<&WasmPluginHandle> {
        self.plugins.values().find(|p| p.has_stage(stage_name))
    }

    /// Find capability for a stage
    pub fn find_capability(&self, stage_name: &str) -> Option<&StageCapability> {
        for plugin in self.plugins.values() {
            if let Some(cap) = plugin.capabilities.iter().find(|c| c.name == stage_name) {
                return Some(cap);
            }
        }
        None
    }

    /// Execute a stage from a WASM plugin
    ///
    /// This is the unified execution interface for all stages.
    pub async fn execute(
        &self,
        plugin_name: &str,
        stage_name: &str,
        context: ExecutionContext,
    ) -> Result<DataFormat> {
        let handle = self
            .plugins
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("WASM plugin '{}' not loaded", plugin_name))?;

        // Verify the plugin provides this stage
        if !handle.has_stage(stage_name) {
            anyhow::bail!(
                "WASM plugin '{}' does not provide stage '{}'",
                plugin_name,
                stage_name
            );
        }

        // Create new store for this execution with file system access
        let current_dir = std::env::current_dir()?;
        let current_dir_str = current_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Current directory path contains invalid UTF-8"))?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .preopened_dir(
                current_dir_str,
                ".",
                wasmtime_wasi::DirPerms::all(),
                wasmtime_wasi::FilePerms::all(),
            )?
            .build();
        let table = ResourceTable::new();
        let state = PluginState { wasi, table };
        let mut store = Store::new(&self.engine, state);

        // Create linker
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Instantiate plugin
        let plugin = Plugin::instantiate_async(&mut store, &handle.component, &linker).await?;

        // Call execute function
        plugin
            .call_execute(&mut store, stage_name, &context)
            .await?
            .map_err(|e| {
                anyhow::anyhow!(
                    "WASM plugin '{}' stage '{}' error: {:?}",
                    plugin_name,
                    stage_name,
                    e
                )
            })
    }

    /// Validate plugin configuration for a stage
    pub async fn validate_config(
        &self,
        plugin_name: &str,
        stage_name: &str,
        config: Vec<(String, String)>,
    ) -> Result<()> {
        let handle = self
            .plugins
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("WASM plugin '{}' not loaded", plugin_name))?;

        // Verify the plugin provides this stage
        if !handle.has_stage(stage_name) {
            anyhow::bail!(
                "WASM plugin '{}' does not provide stage '{}'",
                plugin_name,
                stage_name
            );
        }

        // Create new store for this execution with file system access
        let current_dir = std::env::current_dir()?;
        let current_dir_str = current_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Current directory path contains invalid UTF-8"))?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .preopened_dir(
                current_dir_str,
                ".",
                wasmtime_wasi::DirPerms::all(),
                wasmtime_wasi::FilePerms::all(),
            )?
            .build();
        let table = ResourceTable::new();
        let state = PluginState { wasi, table };
        let mut store = Store::new(&self.engine, state);

        // Create linker
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Instantiate plugin
        let plugin = Plugin::instantiate_async(&mut store, &handle.component, &linker).await?;

        // Call validate-config function
        plugin
            .call_validate_config(&mut store, stage_name, &config)
            .await?
            .map_err(|e| {
                anyhow::anyhow!(
                    "WASM plugin '{}' stage '{}' config validation error: {:?}",
                    plugin_name,
                    stage_name,
                    e
                )
            })
    }
}

impl Default for WasmPluginLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create WASM plugin loader")
    }
}

/// Get WASM plugin search paths in priority order
fn get_wasm_plugin_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. System-wide WASM plugins: ~/.conveyor/wasm-plugins
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".conveyor").join("wasm-plugins"));
    }

    // 2. Development WASM plugins: ./target/wasm32-wasip2/release
    paths.push(PathBuf::from("target/wasm32-wasip2/release"));

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_plugin_loader_creation() {
        let loader = WasmPluginLoader::new();
        assert!(loader.is_ok());
    }

    #[test]
    fn test_custom_plugin_dir() {
        let loader = WasmPluginLoader::new()
            .unwrap()
            .with_plugin_dir("/custom/path");
        assert_eq!(loader.plugin_dir, PathBuf::from("/custom/path"));
    }
}
