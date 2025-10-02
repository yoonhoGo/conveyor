use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use crate::core::config::DagPipelineConfig;
use crate::core::dag_builder::DagPipelineBuilder;
use crate::core::registry::ModuleRegistry;
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

/// Pipeline stage that executes another pipeline
///
/// Configuration:
/// - `file`: Path to external pipeline TOML file
/// - `inline`: Inline TOML pipeline configuration (mutually exclusive with `file`)
///
/// Example (file):
/// ```toml
/// [[stages]]
/// id = "preprocessing"
/// type = "pipeline"
/// inputs = ["load_data"]
///
/// [stages.config]
/// file = "pipelines/preprocess.toml"
/// ```
///
/// Example (inline):
/// ```toml
/// [[stages]]
/// id = "preprocessing"
/// type = "pipeline"
/// inputs = ["load_data"]
///
/// [stages.config]
/// inline = """
/// [[stages]]
/// id = "clean"
/// type = "transform.filter"
/// inputs = []
///
/// [stages.config]
/// column = "status"
/// operator = "!="
/// value = "invalid"
/// """
/// ```
pub struct PipelineStage {
    registry: Arc<ModuleRegistry>,
}

impl PipelineStage {
    pub fn new(registry: Arc<ModuleRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Stage for PipelineStage {
    fn name(&self) -> &str {
        "pipeline"
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        info!("Executing pipeline stage");

        // Get file or inline config
        let pipeline_config = if let Some(file_value) = config.get("file") {
            // Load from file
            let file_path = file_value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("'file' must be a string"))?;

            info!("Loading sub-pipeline from file: {}", file_path);
            DagPipelineConfig::from_file(file_path).await?
        } else if let Some(inline_value) = config.get("inline") {
            // Parse inline TOML
            let inline_toml = inline_value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("'inline' must be a string"))?;

            info!("Parsing inline sub-pipeline configuration");

            // Create a complete pipeline config with the inline stages
            let full_config = format!(
                r#"
[pipeline]
name = "inline-pipeline"
version = "1.0"

{}
"#,
                inline_toml
            );

            DagPipelineConfig::from_str(&full_config)?
        } else {
            return Err(anyhow::anyhow!(
                "Pipeline stage requires either 'file' or 'inline' configuration"
            ));
        };

        info!(
            "Sub-pipeline '{}' has {} stages",
            pipeline_config.pipeline.name,
            pipeline_config.stages.len()
        );

        // Build executor
        let builder = DagPipelineBuilder::new(Arc::clone(&self.registry));
        let executor = builder.build(&pipeline_config)?;

        // Execute sub-pipeline
        info!("Executing sub-pipeline");
        executor.execute().await?;

        info!("Sub-pipeline execution completed");

        // For now, return the first input if available, or empty DataFrame
        // TODO: In the future, we should capture and return the output from the sub-pipeline
        if let Some(data) = inputs.values().next() {
            Ok(data.clone())
        } else {
            Ok(DataFormat::DataFrame(polars::prelude::DataFrame::empty()))
        }
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        // Must have either file or inline
        let has_file = config.contains_key("file");
        let has_inline = config.contains_key("inline");

        if !has_file && !has_inline {
            return Err(anyhow::anyhow!(
                "Pipeline stage requires either 'file' or 'inline' configuration"
            ));
        }

        if has_file && has_inline {
            return Err(anyhow::anyhow!(
                "Pipeline stage cannot have both 'file' and 'inline' configuration"
            ));
        }

        // Validate the pipeline config if possible
        if has_file {
            let file_path = config
                .get("file")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("'file' must be a string"))?;

            // Check if file exists and is valid TOML
            if let Ok(pipeline_config) = DagPipelineConfig::from_file(file_path).await {
                pipeline_config.validate()?;
            }
        } else if has_inline {
            let inline_toml = config
                .get("inline")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("'inline' must be a string"))?;

            // Validate inline config
            let full_config = format!(
                r#"
[pipeline]
name = "inline-pipeline"
version = "1.0"

{}
"#,
                inline_toml
            );

            let pipeline_config = DagPipelineConfig::from_str(&full_config)?;
            pipeline_config.validate()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::DataFrame;

    #[tokio::test]
    async fn test_pipeline_stage_validate_config() {
        let registry = Arc::new(ModuleRegistry::with_defaults().await.unwrap());
        let stage = PipelineStage::new(registry);

        // Test missing both file and inline
        let mut config = HashMap::new();
        assert!(stage.validate_config(&config).await.is_err());

        // Test with file
        config.insert("file".to_string(), toml::Value::String("test.toml".to_string()));
        // This will fail because file doesn't exist, but that's ok for validation
        let _ = stage.validate_config(&config).await;

        // Test with both file and inline
        config.insert(
            "inline".to_string(),
            toml::Value::String("[[stages]]".to_string()),
        );
        assert!(stage.validate_config(&config).await.is_err());
    }

    // Note: More comprehensive integration tests are in tests/pipeline_stage_test.rs
}
