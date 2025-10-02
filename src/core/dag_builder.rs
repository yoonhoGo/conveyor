use anyhow::Result;
use std::sync::Arc;

use crate::core::config::{DagPipelineConfig, StageConfig};
use crate::core::dag_executor::DagExecutor;
use crate::core::error::ConveyorError;
use crate::core::registry::ModuleRegistry;
use crate::core::stage::{
    FfiPluginStageAdapter, SinkStageAdapter, SourceStageAdapter, StageRef, TransformStageAdapter,
    WasmPluginStageAdapter,
};
use crate::plugin_loader::PluginLoader;
use crate::wasm_plugin_loader::WasmPluginLoader;

/// Builder for constructing DAG pipelines from configuration
pub struct DagPipelineBuilder {
    registry: Arc<ModuleRegistry>,
    plugin_loader: Option<Arc<PluginLoader>>,
    wasm_plugin_loader: Option<Arc<WasmPluginLoader>>,
}

impl DagPipelineBuilder {
    pub fn new(registry: Arc<ModuleRegistry>) -> Self {
        Self {
            registry,
            plugin_loader: None,
            wasm_plugin_loader: None,
        }
    }

    /// Add FFI plugin loader
    pub fn with_plugin_loader(mut self, loader: Arc<PluginLoader>) -> Self {
        self.plugin_loader = Some(loader);
        self
    }

    /// Add WASM plugin loader
    pub fn with_wasm_plugin_loader(mut self, loader: Arc<WasmPluginLoader>) -> Self {
        self.wasm_plugin_loader = Some(loader);
        self
    }

    /// Build a DAG executor from configuration
    pub fn build(&self, config: &DagPipelineConfig) -> Result<DagExecutor> {
        let error_strategy = config.error_handling.strategy.clone();
        let mut executor = DagExecutor::new(error_strategy);

        // Create stages and add to executor
        for stage_config in &config.stages {
            let stage = self.create_stage(stage_config)?;
            executor.add_stage(
                stage_config.id.clone(),
                stage,
                stage_config.config.clone(),
            )?;
        }

        // Add dependencies
        for stage_config in &config.stages {
            for input_id in &stage_config.inputs {
                executor.add_dependency(input_id, &stage_config.id)?;
            }
        }

        // Validate the DAG (check for cycles)
        executor.validate()?;

        Ok(executor)
    }

    /// Create a stage from configuration
    ///
    /// Supports three formats:
    /// 1. Built-in: "source.json", "transform.filter", "sink.csv"
    /// 2. FFI Plugin: "plugin.http" (auto-detects from loaded plugins)
    /// 3. WASM Plugin: "wasm.echo" (auto-detects from loaded WASM plugins)
    fn create_stage(&self, stage_config: &StageConfig) -> Result<StageRef> {
        // Parse stage type
        let parts: Vec<&str> = stage_config.stage_type.split('.').collect();

        if parts.len() != 2 {
            return Err(ConveyorError::PipelineError(format!(
                "Invalid stage type format: '{}'. Expected 'category.name' (e.g., 'source.json', 'plugin.http', 'wasm.echo')",
                stage_config.stage_type
            ))
            .into());
        }

        let category = parts[0];
        let type_name = parts[1];

        match category {
            // Built-in source
            "source" => {
                let source = self.registry.get_source(type_name).ok_or_else(|| {
                    ConveyorError::ModuleNotFound(format!("Source type '{}' not found", type_name))
                })?;

                Ok(Arc::new(SourceStageAdapter::new(
                    stage_config.id.clone(),
                    Arc::clone(source),
                )))
            }

            // Built-in transform
            "transform" => {
                let transform = self
                    .registry
                    .get_transform(type_name)
                    .ok_or_else(|| {
                        ConveyorError::ModuleNotFound(format!(
                            "Transform type '{}' not found",
                            type_name
                        ))
                    })?;

                Ok(Arc::new(TransformStageAdapter::new(
                    stage_config.id.clone(),
                    Arc::clone(transform),
                )))
            }

            // Built-in sink
            "sink" => {
                let sink = self.registry.get_sink(type_name).ok_or_else(|| {
                    ConveyorError::ModuleNotFound(format!("Sink type '{}' not found", type_name))
                })?;

                Ok(Arc::new(SinkStageAdapter::new(
                    stage_config.id.clone(),
                    Arc::clone(sink),
                )))
            }

            // FFI Plugin stage
            "plugin" => {
                let loader = self.plugin_loader.as_ref().ok_or_else(|| {
                    ConveyorError::PipelineError(
                        "FFI plugin loader not initialized. Add plugins to configuration.".to_string()
                    )
                })?;

                // Create stage from plugin
                let stage_instance = loader
                    .create_stage(type_name)
                    .map_err(|e| ConveyorError::PipelineError(format!("Failed to create FFI plugin stage '{}': {}", type_name, e)))?;

                Ok(Arc::new(FfiPluginStageAdapter::new(
                    stage_config.id.clone(),
                    type_name.to_string(),
                    stage_instance,
                )))
            }

            // WASM Plugin stage
            "wasm" => {
                let loader = self.wasm_plugin_loader.as_ref().ok_or_else(|| {
                    ConveyorError::PipelineError(
                        "WASM plugin loader not initialized. Add WASM plugins to configuration.".to_string()
                    )
                })?;

                // Find which plugin provides this stage
                let plugin = loader.find_plugin_for_stage(type_name).ok_or_else(|| {
                    ConveyorError::ModuleNotFound(format!("WASM stage '{}' not found in any loaded plugin", type_name))
                })?;

                Ok(Arc::new(WasmPluginStageAdapter::new(
                    stage_config.id.clone(),
                    plugin.name().to_string(),
                    type_name.to_string(),
                    Arc::clone(loader),
                )))
            }

            _ => Err(ConveyorError::PipelineError(format!(
                "Invalid stage category: '{}'. Must be 'source', 'transform', 'sink', 'plugin', or 'wasm'",
                category
            ))
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dag_builder() {
        let registry = Arc::new(ModuleRegistry::with_defaults().await.unwrap());
        let builder = DagPipelineBuilder::new(registry);

        let config_str = r#"
[pipeline]
name = "test"
version = "1.0"

[[stages]]
id = "load_data"
type = "source.json"
inputs = []

[stages.config]
path = "test.json"

[[stages]]
id = "filter_data"
type = "transform.filter"
inputs = ["load_data"]

[stages.config]
column = "status"
operator = "=="
value = "active"

[[stages]]
id = "save_data"
type = "sink.json"
inputs = ["filter_data"]

[stages.config]
path = "output.json"
"#;

        let config = DagPipelineConfig::from_str(config_str).unwrap();
        let executor = builder.build(&config);

        assert!(executor.is_ok());
    }

    #[tokio::test]
    async fn test_dag_builder_invalid_stage_type() {
        let registry = Arc::new(ModuleRegistry::with_defaults().await.unwrap());
        let builder = DagPipelineBuilder::new(registry);

        let config_str = r#"
[pipeline]
name = "test"
version = "1.0"

[[stages]]
id = "invalid"
type = "invalid_format"
inputs = []
"#;

        let config = DagPipelineConfig::from_str(config_str).unwrap();
        let executor = builder.build(&config);

        assert!(executor.is_err());
    }
}
