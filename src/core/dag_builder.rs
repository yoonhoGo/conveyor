use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::config::{DagPipelineConfig, ExecutorType, StageConfig};
use crate::core::dag_executor::{AsyncPipeline, ChannelDagExecutor, DagExecutor};
use crate::core::error::ConveyorError;
use crate::core::registry::ModuleRegistry;
use crate::core::stage::{FfiPluginStageAdapter, StageRef, WasmPluginStageAdapter};
use crate::plugin_loader::PluginLoader;
use crate::wasm_plugin_loader::WasmPluginLoader;

/// Enum to hold different executor types
pub enum ExecutorVariant {
    Dag(DagExecutor),
    Channel(ChannelDagExecutor),
    Async(AsyncPipeline),
}

impl ExecutorVariant {
    /// Validate the executor
    pub fn validate(&self) -> Result<()> {
        match self {
            ExecutorVariant::Dag(e) => e.validate(),
            ExecutorVariant::Channel(e) => e.validate(),
            ExecutorVariant::Async(e) => e.validate(),
        }
    }

    /// Execute the executor
    pub async fn execute(&mut self) -> Result<()> {
        match self {
            ExecutorVariant::Dag(e) => e.execute().await,
            ExecutorVariant::Channel(e) => e.execute().await,
            ExecutorVariant::Async(e) => e.execute().await,
        }
    }
}

/// Common trait for all executor types
trait ExecutorBuilder {
    fn add_stage(
        &mut self,
        id: String,
        stage: StageRef,
        config: HashMap<String, toml::Value>,
    ) -> Result<()>;
    fn add_dependency(&mut self, from_id: &str, to_id: &str) -> Result<()>;
    fn validate(&self) -> Result<()>;
}

impl ExecutorBuilder for DagExecutor {
    fn add_stage(
        &mut self,
        id: String,
        stage: StageRef,
        config: HashMap<String, toml::Value>,
    ) -> Result<()> {
        self.add_stage(id, stage, config)
    }

    fn add_dependency(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        self.add_dependency(from_id, to_id)
    }

    fn validate(&self) -> Result<()> {
        self.validate()
    }
}

impl ExecutorBuilder for ChannelDagExecutor {
    fn add_stage(
        &mut self,
        id: String,
        stage: StageRef,
        config: HashMap<String, toml::Value>,
    ) -> Result<()> {
        self.add_stage(id, stage, config)
    }

    fn add_dependency(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        self.add_dependency(from_id, to_id)
    }

    fn validate(&self) -> Result<()> {
        self.validate()
    }
}

impl ExecutorBuilder for AsyncPipeline {
    fn add_stage(
        &mut self,
        id: String,
        stage: StageRef,
        config: HashMap<String, toml::Value>,
    ) -> Result<()> {
        self.add_stage(id, stage, config)
    }

    fn add_dependency(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        self.add_dependency(from_id, to_id)
    }

    fn validate(&self) -> Result<()> {
        self.validate()
    }
}

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
    pub fn build(&self, config: &DagPipelineConfig) -> Result<ExecutorVariant> {
        let error_strategy = config.error_handling.strategy.clone();
        let executor_type = config.global.executor;

        match executor_type {
            ExecutorType::Dag => {
                let mut executor = DagExecutor::new(error_strategy);
                self.build_stages(&mut executor, config)?;
                Ok(ExecutorVariant::Dag(executor))
            }
            ExecutorType::Channel => {
                let buffer_size = config.global.channel_buffer_size;
                let concurrency = config.global.concurrency;
                let mut executor = ChannelDagExecutor::new(error_strategy, buffer_size, concurrency);
                self.build_stages(&mut executor, config)?;
                Ok(ExecutorVariant::Channel(executor))
            }
            ExecutorType::Async => {
                let buffer_size = config.global.channel_buffer_size;
                let mut executor = AsyncPipeline::new(error_strategy, buffer_size);
                self.build_stages(&mut executor, config)?;
                Ok(ExecutorVariant::Async(executor))
            }
        }
    }

    /// Build stages and dependencies for any executor type
    fn build_stages<E: ExecutorBuilder>(
        &self,
        executor: &mut E,
        config: &DagPipelineConfig,
    ) -> Result<()> {
        // Create stages and add to executor
        for stage_config in &config.stages {
            let stage = self.create_stage(stage_config)?;
            executor.add_stage(stage_config.id.clone(), stage, stage_config.config.clone())?;
        }

        // Add dependencies
        for stage_config in &config.stages {
            for input_id in &stage_config.inputs {
                executor.add_dependency(input_id, &stage_config.id)?;
            }
        }

        // Validate the DAG (check for cycles)
        executor.validate()?;

        Ok(())
    }

    /// Create a stage from configuration using function-based API
    ///
    /// Supports:
    /// - Built-in functions: "csv.read", "json.write", "filter.apply"
    /// - Plugin functions: "mongodb-find", "http-get" (from FFI/WASM plugins)
    /// - Special stages: "stage.pipeline"
    fn create_stage(&self, stage_config: &StageConfig) -> Result<StageRef> {
        let function_name = &stage_config.function;

        // 1. Try registry lookup (built-in functions)
        if let Some(stage) = self.registry.get_function(function_name) {
            tracing::debug!(
                "Found function '{}' in registry for stage '{}'",
                function_name,
                stage_config.id
            );
            return Ok(Arc::clone(stage));
        }

        // 2. Try FFI plugin lookup
        if let Some(loader) = &self.plugin_loader {
            if let Some(capability) = loader.find_capability(function_name) {
                let stage_instance = loader.create_stage(function_name)?;
                tracing::debug!(
                    "Found function '{}' in FFI plugins for stage '{}'",
                    function_name,
                    stage_config.id
                );
                return Ok(Arc::new(FfiPluginStageAdapter::new(
                    stage_config.id.clone(),
                    function_name.to_string(),
                    capability.description.to_string(),
                    capability.stage_type,
                    stage_instance,
                )));
            }
        }

        // 3. Try WASM plugin lookup
        if let Some(loader) = &self.wasm_plugin_loader {
            if let Some(capability) = loader.find_capability(function_name) {
                if let Some(plugin) = loader.find_plugin_for_stage(function_name) {
                    tracing::debug!(
                        "Found function '{}' in WASM plugins for stage '{}'",
                        function_name,
                        stage_config.id
                    );
                    // Convert stage type to string
                    use crate::wasm_plugin_loader::StageType as WasmStageType;
                    let stage_type_str = match capability.stage_type {
                        WasmStageType::Source => "source",
                        WasmStageType::Transform => "transform",
                        WasmStageType::Sink => "sink",
                    };

                    return Ok(Arc::new(WasmPluginStageAdapter::new(
                        stage_config.id.clone(),
                        plugin.name().to_string(),
                        function_name.to_string(),
                        capability.description.clone(),
                        stage_type_str.to_string(),
                        Arc::clone(loader),
                    )));
                }
            }
        }

        // 4. Special stage: "stage.pipeline"
        if function_name == "stage.pipeline" {
            use crate::modules::stages::PipelineStage;
            return Ok(Arc::new(PipelineStage::new(Arc::clone(&self.registry))));
        }

        // Not found
        Err(ConveyorError::ModuleNotFound(format!(
            "Function '{}' not found in registry or plugins",
            function_name
        ))
        .into())
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
function = "json.read"
inputs = []

[stages.config]
path = "test.json"

[[stages]]
id = "filter_data"
function = "filter.apply"
inputs = ["load_data"]

[stages.config]
column = "status"
operator = "=="
value = "active"

[[stages]]
id = "save_data"
function = "json.write"
inputs = ["filter_data"]

[stages.config]
path = "output.json"
"#;

        let config = DagPipelineConfig::from_str(config_str).unwrap();
        let result = builder.build(&config);

        assert!(result.is_ok());
        if let Ok(executor) = result {
            assert!(executor.validate().is_ok());
        }
    }

    #[tokio::test]
    async fn test_dag_builder_invalid_function() {
        let registry = Arc::new(ModuleRegistry::with_defaults().await.unwrap());
        let builder = DagPipelineBuilder::new(registry);

        let config_str = r#"
[pipeline]
name = "test"
version = "1.0"

[[stages]]
id = "invalid"
function = "nonexistent.function"
inputs = []
"#;

        let config = DagPipelineConfig::from_str(config_str).unwrap();
        let result = builder.build(&config);

        assert!(result.is_err());
    }
}
