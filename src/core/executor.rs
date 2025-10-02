use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};

use crate::core::config::{SinkConfig, SourceConfig, TransformConfig};
use crate::core::error::ConveyorError;
use crate::core::registry::ModuleRegistry;
use crate::core::strategy::ErrorStrategy;
use crate::core::traits::DataFormat;

/// Executor for pipeline stages (sources, transforms, sinks)
pub struct StageExecutor {
    registry: Arc<ModuleRegistry>,
    error_strategy: ErrorStrategy,
}

impl StageExecutor {
    pub fn new(registry: Arc<ModuleRegistry>, error_strategy: ErrorStrategy) -> Self {
        Self {
            registry,
            error_strategy,
        }
    }

    /// Execute a source and return the data
    pub async fn execute_source(&self, config: &SourceConfig) -> Result<DataFormat> {
        let operation = || async {
            let source = self
                .registry
                .get_source(&config.source_type)
                .ok_or_else(|| {
                    ConveyorError::ModuleNotFound(format!(
                        "Source type '{}' not found",
                        config.source_type
                    ))
                })?;

            // Validate configuration
            source.validate_config(&config.config).await?;

            // Execute source
            let data = source.read(&config.config).await?;

            Ok(data)
        };

        match self
            .error_strategy
            .execute(&format!("source '{}'", config.name), operation)
            .await
        {
            Ok(data) => {
                info!("Source '{}' produced data", config.name);
                Ok(data)
            }
            Err(e) => {
                if self.error_strategy.should_continue_on_error() {
                    warn!("Source '{}' failed: {}. Continuing...", config.name, e);
                    // Return empty DataFrame as fallback
                    Ok(DataFormat::DataFrame(polars::prelude::DataFrame::empty()))
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Execute a transform on the given data
    pub async fn execute_transform(
        &self,
        config: &TransformConfig,
        data: DataFormat,
    ) -> Result<DataFormat> {
        let function_name = config
            .function
            .split('.')
            .next()
            .unwrap_or(&config.function);

        let operation = || async {
            let transform = self.registry.get_transform(function_name).ok_or_else(|| {
                ConveyorError::ModuleNotFound(format!(
                    "Transform function '{}' not found",
                    function_name
                ))
            })?;

            // Validate configuration
            transform.validate_config(&config.config).await?;

            // Execute transform
            let transformed_data = transform.apply(data.clone(), &config.config).await?;

            Ok(transformed_data)
        };

        match self
            .error_strategy
            .execute(&format!("transform '{}'", config.name), operation)
            .await
        {
            Ok(transformed_data) => {
                info!("Transform '{}' completed", config.name);
                Ok(transformed_data)
            }
            Err(e) => {
                if self.error_strategy.should_continue_on_error() {
                    warn!(
                        "Transform '{}' failed: {}. Continuing with original data...",
                        config.name, e
                    );
                    // Return original data as fallback
                    Ok(data)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Execute a sink with the given data
    pub async fn execute_sink(&self, config: &SinkConfig, data: DataFormat) -> Result<()> {
        let operation = || async {
            let sink = self.registry.get_sink(&config.sink_type).ok_or_else(|| {
                ConveyorError::ModuleNotFound(format!("Sink type '{}' not found", config.sink_type))
            })?;

            // Validate configuration
            sink.validate_config(&config.config).await?;

            // Execute sink
            sink.write(data.clone(), &config.config).await?;

            Ok(())
        };

        match self
            .error_strategy
            .execute(&format!("sink '{}'", config.name), operation)
            .await
        {
            Ok(()) => {
                info!("Sink '{}' completed", config.name);
                Ok(())
            }
            Err(e) => {
                if self.error_strategy.should_continue_on_error() {
                    warn!("Sink '{}' failed: {}. Continuing...", config.name, e);
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Helper to validate all stages before execution
pub struct StageValidator {
    registry: Arc<ModuleRegistry>,
}

impl StageValidator {
    pub fn new(registry: Arc<ModuleRegistry>) -> Self {
        Self { registry }
    }

    pub async fn validate_source(&self, config: &SourceConfig) -> Result<()> {
        let source = self
            .registry
            .get_source(&config.source_type)
            .ok_or_else(|| {
                ConveyorError::ModuleNotFound(format!(
                    "Source type '{}' not found",
                    config.source_type
                ))
            })?;

        source.validate_config(&config.config).await
    }

    pub async fn validate_transform(&self, config: &TransformConfig) -> Result<()> {
        let function_name = config
            .function
            .split('.')
            .next()
            .unwrap_or(&config.function);

        let transform = self.registry.get_transform(function_name).ok_or_else(|| {
            ConveyorError::ModuleNotFound(format!(
                "Transform function '{}' not found",
                function_name
            ))
        })?;

        transform.validate_config(&config.config).await
    }

    pub async fn validate_sink(&self, config: &SinkConfig) -> Result<()> {
        let sink = self.registry.get_sink(&config.sink_type).ok_or_else(|| {
            ConveyorError::ModuleNotFound(format!("Sink type '{}' not found", config.sink_type))
        })?;

        sink.validate_config(&config.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stage_executor_creation() {
        let registry = Arc::new(ModuleRegistry::with_defaults().await.unwrap());
        let strategy = ErrorStrategy::Stop;
        let _executor = StageExecutor::new(registry, strategy);
        // Just verify it compiles
    }

    #[tokio::test]
    async fn test_stage_validator_creation() {
        let registry = Arc::new(ModuleRegistry::with_defaults().await.unwrap());
        let _validator = StageValidator::new(registry);
        // Just verify it compiles
    }
}
