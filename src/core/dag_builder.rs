use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::config::{DagPipelineConfig, StageConfig};
use crate::core::dag_executor::DagExecutor;
use crate::core::error::ConveyorError;
use crate::core::registry::ModuleRegistry;
use crate::core::stage::{SinkStageAdapter, SourceStageAdapter, StageRef, TransformStageAdapter};
use crate::core::strategy::ErrorStrategy;

/// Builder for constructing DAG pipelines from configuration
pub struct DagPipelineBuilder {
    registry: Arc<ModuleRegistry>,
}

impl DagPipelineBuilder {
    pub fn new(registry: Arc<ModuleRegistry>) -> Self {
        Self { registry }
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
    fn create_stage(&self, stage_config: &StageConfig) -> Result<StageRef> {
        // Parse stage type: "category.name" (e.g., "source.json", "transform.filter", "sink.csv")
        let parts: Vec<&str> = stage_config.stage_type.split('.').collect();

        if parts.len() != 2 {
            return Err(ConveyorError::PipelineError(format!(
                "Invalid stage type format: '{}'. Expected 'category.name' (e.g., 'source.json')",
                stage_config.stage_type
            ))
            .into());
        }

        let category = parts[0];
        let type_name = parts[1];

        match category {
            "source" => {
                let source = self.registry.get_source(type_name).ok_or_else(|| {
                    ConveyorError::ModuleNotFound(format!("Source type '{}' not found", type_name))
                })?;

                Ok(Arc::new(SourceStageAdapter::new(
                    stage_config.id.clone(),
                    Arc::clone(source),
                )))
            }
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
            "sink" => {
                let sink = self.registry.get_sink(type_name).ok_or_else(|| {
                    ConveyorError::ModuleNotFound(format!("Sink type '{}' not found", type_name))
                })?;

                Ok(Arc::new(SinkStageAdapter::new(
                    stage_config.id.clone(),
                    Arc::clone(sink),
                )))
            }
            _ => Err(ConveyorError::PipelineError(format!(
                "Invalid stage category: '{}'. Must be 'source', 'transform', or 'sink'",
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
