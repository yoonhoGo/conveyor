use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, warn};

use crate::core::config::{PipelineConfig, SourceConfig, TransformConfig, SinkConfig};
use crate::core::error::ConveyorError;
use crate::core::registry::ModuleRegistry;
use crate::core::traits::DataFormat;

pub struct Pipeline {
    config: PipelineConfig,
    registry: Arc<ModuleRegistry>,
}

impl Pipeline {
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = PipelineConfig::from_file(path).await?;
        Ok(Self::new(config))
    }

    pub fn new(config: PipelineConfig) -> Self {
        let registry = Arc::new(ModuleRegistry::with_defaults());
        Self { config, registry }
    }

    pub fn validate(&self) -> Result<()> {
        // Validate configuration
        self.config.validate()?;

        // Validate all sources exist
        for source in &self.config.sources {
            if self.registry.get_source(&source.source_type).is_none() {
                return Err(ConveyorError::ModuleNotFound(
                    format!("Source type '{}' not found", source.source_type)
                ).into());
            }
        }

        // Validate all transforms exist
        for transform in &self.config.transforms {
            let function_name = transform.function.split('.').next().unwrap_or(&transform.function);
            if self.registry.get_transform(function_name).is_none() {
                return Err(ConveyorError::ModuleNotFound(
                    format!("Transform function '{}' not found", function_name)
                ).into());
            }
        }

        // Validate all sinks exist
        for sink in &self.config.sinks {
            if self.registry.get_sink(&sink.sink_type).is_none() {
                return Err(ConveyorError::ModuleNotFound(
                    format!("Sink type '{}' not found", sink.sink_type)
                ).into());
            }
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
                info!("Pipeline '{}' completed successfully", self.config.pipeline.name);
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Pipeline '{}' failed: {}", self.config.pipeline.name, e);
                Err(e)
            }
            Err(_) => {
                error!("Pipeline '{}' timed out after {} seconds",
                    self.config.pipeline.name,
                    self.config.global.timeout_seconds
                );
                Err(ConveyorError::PipelineError(
                    format!("Pipeline timed out after {} seconds", self.config.global.timeout_seconds)
                ).into())
            }
        }
    }

    async fn execute_internal(&self, continue_on_error: bool) -> Result<()> {
        let mut data_map: HashMap<String, DataFormat> = HashMap::new();

        // Process sources
        for source_config in &self.config.sources {
            match self.execute_source(source_config).await {
                Ok(data) => {
                    info!("Source '{}' produced data", source_config.name);
                    data_map.insert(source_config.name.clone(), data);
                }
                Err(e) => {
                    if continue_on_error {
                        warn!("Source '{}' failed: {}. Continuing...", source_config.name, e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        // Process transforms
        for transform_config in &self.config.transforms {
            // Get input data
            let input_name = transform_config.input.as_ref()
                .unwrap_or(&self.config.sources.last().unwrap().name);

            let input_data = data_map.get(input_name)
                .ok_or_else(|| ConveyorError::PipelineError(
                    format!("Input '{}' not found for transform '{}'", input_name, transform_config.name)
                ))?;

            match self.execute_transform(transform_config, input_data.clone()).await {
                Ok(transformed_data) => {
                    info!("Transform '{}' completed", transform_config.name);
                    data_map.insert(transform_config.name.clone(), transformed_data);
                }
                Err(e) => {
                    if continue_on_error {
                        warn!("Transform '{}' failed: {}. Continuing...", transform_config.name, e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        // Process sinks
        for sink_config in &self.config.sinks {
            // Get input data
            let input_name = sink_config.input.as_ref()
                .unwrap_or_else(|| {
                    if !self.config.transforms.is_empty() {
                        &self.config.transforms.last().unwrap().name
                    } else {
                        &self.config.sources.last().unwrap().name
                    }
                });

            let input_data = data_map.get(input_name)
                .ok_or_else(|| ConveyorError::PipelineError(
                    format!("Input '{}' not found for sink '{}'", input_name, sink_config.name)
                ))?;

            match self.execute_sink(sink_config, input_data.clone()).await {
                Ok(()) => {
                    info!("Sink '{}' completed", sink_config.name);
                }
                Err(e) => {
                    if continue_on_error {
                        warn!("Sink '{}' failed: {}. Continuing...", sink_config.name, e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute_source(&self, config: &SourceConfig) -> Result<DataFormat> {
        let source = self.registry.get_source(&config.source_type)
            .ok_or_else(|| ConveyorError::ModuleNotFound(
                format!("Source type '{}' not found", config.source_type)
            ))?;

        // Validate configuration
        source.validate_config(&config.config).await?;

        // Execute source
        let data = source.read(&config.config).await?;

        Ok(data)
    }

    async fn execute_transform(&self, config: &TransformConfig, data: DataFormat) -> Result<DataFormat> {
        let function_name = config.function.split('.').next().unwrap_or(&config.function);

        let transform = self.registry.get_transform(function_name)
            .ok_or_else(|| ConveyorError::ModuleNotFound(
                format!("Transform function '{}' not found", function_name)
            ))?;

        // Validate configuration
        transform.validate_config(&config.config).await?;

        // Execute transform
        let transformed_data = transform.apply(data, &config.config).await?;

        Ok(transformed_data)
    }

    async fn execute_sink(&self, config: &SinkConfig, data: DataFormat) -> Result<()> {
        let sink = self.registry.get_sink(&config.sink_type)
            .ok_or_else(|| ConveyorError::ModuleNotFound(
                format!("Sink type '{}' not found", config.sink_type)
            ))?;

        // Validate configuration
        sink.validate_config(&config.config).await?;

        // Execute sink
        sink.write(data, &config.config).await?;

        Ok(())
    }
}