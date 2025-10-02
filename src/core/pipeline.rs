use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info};

use crate::core::config::{DagPipelineConfig, PipelineConfig};
use crate::core::dag_builder::DagPipelineBuilder;
use crate::core::dag_executor::DagExecutor;
use crate::core::error::ConveyorError;
use crate::core::executor::{StageExecutor, StageValidator};
use crate::core::legacy_converter::LegacyConfigConverter;
use crate::core::registry::ModuleRegistry;
use crate::core::traits::DataFormat;
use crate::plugin_loader::PluginLoader;

pub struct Pipeline {
    config: PipelineConfig,
    registry: Arc<ModuleRegistry>,
    #[allow(dead_code)]
    plugin_loader: PluginLoader,
}

impl Pipeline {
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = PipelineConfig::from_file(path).await?;
        Self::new(config).await
    }

    pub async fn new(config: PipelineConfig) -> Result<Self> {
        let registry = Arc::new(ModuleRegistry::with_defaults().await?);

        // Load plugins specified in config
        let mut plugin_loader = PluginLoader::new();
        if !config.global.plugins.is_empty() {
            info!(
                "Loading {} plugin(s): {:?}",
                config.global.plugins.len(),
                config.global.plugins
            );
            plugin_loader.load_plugins(&config.global.plugins)?;
        }

        Ok(Self {
            config,
            registry,
            plugin_loader,
        })
    }

    pub async fn validate(&self) -> Result<()> {
        // Validate configuration
        self.config.validate()?;

        let validator = StageValidator::new(self.registry.clone());

        // Validate all sources
        for source in &self.config.sources {
            validator.validate_source(source).await?;
        }

        // Validate all transforms
        for transform in &self.config.transforms {
            validator.validate_transform(transform).await?;
        }

        // Validate all sinks
        for sink in &self.config.sinks {
            validator.validate_sink(sink).await?;
        }

        Ok(())
    }

    pub async fn execute(&self, continue_on_error: bool) -> Result<()> {
        info!("Starting pipeline: {}", self.config.pipeline.name);

        let timeout_duration = Duration::from_secs(self.config.global.timeout_seconds);

        // Execute pipeline with timeout
        let result = timeout(timeout_duration, self.execute_internal(continue_on_error)).await;

        match result {
            Ok(Ok(())) => {
                info!(
                    "Pipeline '{}' completed successfully",
                    self.config.pipeline.name
                );
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Pipeline '{}' failed: {}", self.config.pipeline.name, e);
                Err(e)
            }
            Err(_) => {
                error!(
                    "Pipeline '{}' timed out after {} seconds",
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

    async fn execute_internal(&self, continue_on_error: bool) -> Result<()> {
        use crate::core::strategy::ErrorStrategy;

        let mut data_map: HashMap<String, DataFormat> = HashMap::new();

        // Convert continue_on_error to ErrorStrategy
        let strategy = if continue_on_error {
            ErrorStrategy::Continue
        } else {
            ErrorStrategy::Stop
        };

        let executor = StageExecutor::new(self.registry.clone(), strategy);

        // Process sources
        for source_config in &self.config.sources {
            let data = executor.execute_source(source_config).await?;
            data_map.insert(source_config.name.clone(), data);
        }

        // Process transforms
        for transform_config in &self.config.transforms {
            // Get input data
            let input_name = transform_config
                .input
                .as_ref()
                .unwrap_or(&self.config.sources.last().unwrap().name);

            let input_data = data_map.get(input_name).ok_or_else(|| {
                ConveyorError::PipelineError(format!(
                    "Input '{}' not found for transform '{}'",
                    input_name, transform_config.name
                ))
            })?;

            let transformed_data = executor
                .execute_transform(transform_config, input_data.clone())
                .await?;
            data_map.insert(transform_config.name.clone(), transformed_data);
        }

        // Process sinks
        for sink_config in &self.config.sinks {
            // Get input data
            let input_name = sink_config.input.as_ref().unwrap_or_else(|| {
                if !self.config.transforms.is_empty() {
                    &self.config.transforms.last().unwrap().name
                } else {
                    &self.config.sources.last().unwrap().name
                }
            });

            let input_data = data_map.get(input_name).ok_or_else(|| {
                ConveyorError::PipelineError(format!(
                    "Input '{}' not found for sink '{}'",
                    input_name, sink_config.name
                ))
            })?;

            executor
                .execute_sink(sink_config, input_data.clone())
                .await?;
        }

        Ok(())
    }
}

// ============================================================================
// DAG-based Pipeline
// ============================================================================

/// DAG-based pipeline supporting flexible stage composition
pub struct DagPipeline {
    config: DagPipelineConfig,
    #[allow(dead_code)]
    registry: Arc<ModuleRegistry>,
    executor: DagExecutor,
    #[allow(dead_code)]
    plugin_loader: PluginLoader,
}

impl DagPipeline {
    /// Create a DAG pipeline from a file
    /// Automatically detects whether the file is in DAG format or legacy format
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;

        // Try to parse as DAG format first
        if let Ok(dag_config) = DagPipelineConfig::from_str(&content) {
            info!("Loading DAG-based pipeline configuration");
            Self::new(dag_config).await
        } else {
            // Fall back to legacy format and convert
            info!("Loading legacy pipeline configuration and converting to DAG format");
            let legacy_config = PipelineConfig::from_str(&content)?;
            let dag_config = LegacyConfigConverter::convert(legacy_config);
            Self::new(dag_config).await
        }
    }

    /// Create a DAG pipeline from configuration
    pub async fn new(config: DagPipelineConfig) -> Result<Self> {
        let registry = Arc::new(ModuleRegistry::with_defaults().await?);

        // Load plugins specified in config
        let mut plugin_loader = PluginLoader::new();
        if !config.global.plugins.is_empty() {
            info!(
                "Loading {} plugin(s): {:?}",
                config.global.plugins.len(),
                config.global.plugins
            );
            plugin_loader.load_plugins(&config.global.plugins)?;
        }

        // Build DAG executor
        let builder = DagPipelineBuilder::new(registry.clone());
        let executor = builder.build(&config)?;

        Ok(Self {
            config,
            registry,
            executor,
            plugin_loader,
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

/// Factory function to create the appropriate pipeline type
pub async fn create_pipeline_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<Box<dyn PipelineExecutor>> {
    let content = tokio::fs::read_to_string(path.as_ref()).await?;

    // Try DAG format first
    if let Ok(dag_config) = DagPipelineConfig::from_str(&content) {
        let pipeline = DagPipeline::new(dag_config).await?;
        Ok(Box::new(pipeline))
    } else {
        // Fall back to legacy format
        let legacy_config = PipelineConfig::from_str(&content)?;
        let pipeline = Pipeline::new(legacy_config).await?;
        Ok(Box::new(pipeline))
    }
}

/// Common interface for pipeline execution
#[async_trait::async_trait]
pub trait PipelineExecutor: Send + Sync {
    async fn execute(&self) -> Result<()>;
}

#[async_trait::async_trait]
impl PipelineExecutor for Pipeline {
    async fn execute(&self) -> Result<()> {
        self.execute(false).await
    }
}

#[async_trait::async_trait]
impl PipelineExecutor for DagPipeline {
    async fn execute(&self) -> Result<()> {
        self.execute().await
    }
}
