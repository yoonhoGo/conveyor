//! WASM Plugin Loader
//!
//! Loads and manages WebAssembly plugins using Wasmtime Component Model.
//!
//! This module provides host-side integration for WASM plugins, complementing
//! the FFI plugin system with enhanced sandboxing and portability.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

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
        // Construct plugin path
        let plugin_path = self.plugin_dir.join(format!("conveyor_plugin_{}.wasm", name));

        if !plugin_path.exists() {
            anyhow::bail!(
                "WASM plugin '{}' not found at {:?}",
                name,
                plugin_path
            );
        }

        tracing::info!("Loading WASM plugin '{}' from {:?}", name, plugin_path);

        // Load component
        let component = Component::from_file(&self.engine, &plugin_path)
            .with_context(|| format!("Failed to load WASM component from {:?}", plugin_path))?;

        // Create store with WASI context
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
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

        tracing::info!(
            "Loaded WASM plugin: {} v{} (API v{})",
            metadata.name,
            metadata.version,
            metadata.api_version
        );

        // Store plugin handle
        let handle = WasmPluginHandle {
            component,
            metadata,
        };

        self.plugins.insert(name.to_string(), handle);

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

    /// Get loaded plugin metadata
    pub fn get_plugin_metadata(&self, name: &str) -> Option<&PluginMetadata> {
        self.plugins.get(name).map(|h| &h.metadata)
    }

    /// List all loaded plugins
    pub fn list_plugins(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    /// Execute read operation on a WASM plugin
    pub async fn read(
        &self,
        plugin_name: &str,
        config: Vec<(String, String)>,
    ) -> Result<DataFormat> {
        let handle = self
            .plugins
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("WASM plugin '{}' not loaded", plugin_name))?;

        // Create new store for this execution
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
        let table = ResourceTable::new();
        let state = PluginState { wasi, table };
        let mut store = Store::new(&self.engine, state);

        // Create linker
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Instantiate plugin
        let plugin = Plugin::instantiate_async(&mut store, &handle.component, &linker).await?;

        // Call read function
        plugin
            .call_read(&mut store, &config)
            .await?
            .map_err(|e| anyhow::anyhow!("Plugin read error: {:?}", e))
    }

    /// Execute write operation on a WASM plugin
    pub async fn write(
        &self,
        plugin_name: &str,
        data: DataFormat,
        config: Vec<(String, String)>,
    ) -> Result<()> {
        let handle = self
            .plugins
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("WASM plugin '{}' not loaded", plugin_name))?;

        // Create new store for this execution
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
        let table = ResourceTable::new();
        let state = PluginState { wasi, table };
        let mut store = Store::new(&self.engine, state);

        // Create linker
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Instantiate plugin
        let plugin = Plugin::instantiate_async(&mut store, &handle.component, &linker).await?;

        // Call write function
        plugin
            .call_write(&mut store, &data, &config)
            .await?
            .map_err(|e| anyhow::anyhow!("Plugin write error: {:?}", e))
    }

    /// Execute transform operation on a WASM plugin
    pub async fn transform(
        &self,
        plugin_name: &str,
        data: DataFormat,
        config: Vec<(String, String)>,
    ) -> Result<DataFormat> {
        let handle = self
            .plugins
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("WASM plugin '{}' not loaded", plugin_name))?;

        // Create new store for this execution
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
        let table = ResourceTable::new();
        let state = PluginState { wasi, table };
        let mut store = Store::new(&self.engine, state);

        // Create linker
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        // Instantiate plugin
        let plugin = Plugin::instantiate_async(&mut store, &handle.component, &linker).await?;

        // Call transform function
        plugin
            .call_transform(&mut store, &data, &config)
            .await?
            .map_err(|e| anyhow::anyhow!("Plugin transform error: {:?}", e))
    }

    /// Validate plugin configuration
    pub async fn validate_config(
        &self,
        plugin_name: &str,
        config: Vec<(String, String)>,
    ) -> Result<()> {
        let handle = self
            .plugins
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("WASM plugin '{}' not loaded", plugin_name))?;

        // Create new store for this execution
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
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
            .call_validate_config(&mut store, &config)
            .await?
            .map_err(|e| anyhow::anyhow!("Plugin config validation error: {:?}", e))
    }
}

impl Default for WasmPluginLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create WASM plugin loader")
    }
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
