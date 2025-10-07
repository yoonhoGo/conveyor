use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info};

use crate::core::config::DagPipelineConfig;
use crate::core::dag_builder::DagPipelineBuilder;
use crate::core::dag_executor::DagExecutor;
use crate::core::error::ConveyorError;
use crate::core::registry::ModuleRegistry;
use crate::plugin_loader::PluginLoader;
use crate::wasm_plugin_loader::WasmPluginLoader;

/// DAG-based pipeline supporting flexible stage composition
pub struct DagPipeline {
    config: DagPipelineConfig,
    #[allow(dead_code)]
    registry: Arc<ModuleRegistry>,
    executor: DagExecutor,
    #[allow(dead_code)]
    plugin_loader: Option<Arc<PluginLoader>>,
    #[allow(dead_code)]
    wasm_plugin_loader: Option<Arc<WasmPluginLoader>>,
}

impl DagPipeline {
    /// Create a DAG pipeline from a file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let dag_config = DagPipelineConfig::from_str(&content)?;
        info!("Loading DAG-based pipeline configuration");
        Self::new(dag_config).await
    }

    /// Create a DAG pipeline from configuration
    pub async fn new(config: DagPipelineConfig) -> Result<Self> {
        let registry = Arc::new(ModuleRegistry::with_defaults().await?);

        // Load FFI plugins specified in config
        let mut plugin_loader = PluginLoader::new();
        if !config.global.plugins.is_empty() {
            info!(
                "Loading {} FFI plugin(s): {:?}",
                config.global.plugins.len(),
                config.global.plugins
            );
            plugin_loader.load_plugins(&config.global.plugins)?;
        }

        // Load WASM plugins specified in config
        let mut wasm_plugin_loader = WasmPluginLoader::new()?;
        if !config.global.wasm_plugins.is_empty() {
            info!(
                "Loading {} WASM plugin(s): {:?}",
                config.global.wasm_plugins.len(),
                config.global.wasm_plugins
            );
            for plugin_name in &config.global.wasm_plugins {
                wasm_plugin_loader.load_plugin(plugin_name).await?;
            }
        }

        // Build DAG executor with plugin loaders
        let plugin_loader_arc = Arc::new(plugin_loader);
        let wasm_plugin_loader_arc = Arc::new(wasm_plugin_loader);
        let builder = DagPipelineBuilder::new(registry.clone())
            .with_plugin_loader(plugin_loader_arc.clone())
            .with_wasm_plugin_loader(wasm_plugin_loader_arc.clone());
        let executor = builder.build(&config)?;

        Ok(Self {
            config,
            registry,
            executor,
            plugin_loader: Some(plugin_loader_arc),
            wasm_plugin_loader: Some(wasm_plugin_loader_arc),
        })
    }

    /// Validate the DAG pipeline
    pub fn validate(&self) -> Result<()> {
        self.executor.validate()
    }

    /// Execute the DAG pipeline
    pub async fn execute(&self) -> Result<()> {
        info!("Starting DAG pipeline: {}", self.config.pipeline.name);

        let timeout_duration = Duration::from_secs(self.config.global.timeout_seconds);

        // Execute pipeline with timeout
        let result = timeout(timeout_duration, self.executor.execute()).await;

        match result {
            Ok(Ok(())) => {
                info!(
                    "DAG pipeline '{}' completed successfully",
                    self.config.pipeline.name
                );
                Ok(())
            }
            Ok(Err(e)) => {
                error!("DAG pipeline '{}' failed: {}", self.config.pipeline.name, e);
                Err(e)
            }
            Err(_) => {
                error!(
                    "DAG pipeline '{}' timed out after {} seconds",
                    self.config.pipeline.name, self.config.global.timeout_seconds
                );
                Err(ConveyorError::PipelineError(format!(
                    "Pipeline timed out after {} seconds",
                    self.config.global.timeout_seconds
                ))
                .into())
            }
        }
    }
}
