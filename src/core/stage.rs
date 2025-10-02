use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::traits::{DataFormat, DataSource, Sink, Transform};

/// Unified Stage trait - represents any processing unit in the pipeline
/// This allows sources, transforms, and sinks to be treated uniformly in a DAG
#[async_trait]
pub trait Stage: Send + Sync {
    /// Unique identifier for this stage type
    fn name(&self) -> &str;

    /// Execute the stage with given inputs and configuration
    ///
    /// # Arguments
    /// * `inputs` - Map of input data from previous stages (can be empty for sources)
    /// * `config` - Stage-specific configuration from TOML
    ///
    /// # Returns
    /// * `Result<DataFormat>` - Processed data to pass to next stages
    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat>;

    /// Validate the stage configuration
    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()>;

    /// Whether this stage produces output (false for sinks)
    fn produces_output(&self) -> bool {
        true
    }
}

pub type StageRef = Arc<dyn Stage>;

/// Adapter to convert DataSource into Stage
pub struct SourceStageAdapter {
    source: Arc<dyn DataSource>,
    name: String,
}

impl SourceStageAdapter {
    pub fn new(name: String, source: Arc<dyn DataSource>) -> Self {
        Self { source, name }
    }
}

#[async_trait]
impl Stage for SourceStageAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(
        &self,
        _inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        // Sources ignore inputs - they fetch data from external sources
        self.source.read(config).await
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        self.source.validate_config(config).await
    }
}

/// Adapter to convert Transform into Stage
pub struct TransformStageAdapter {
    transform: Arc<dyn Transform>,
    name: String,
}

impl TransformStageAdapter {
    pub fn new(name: String, transform: Arc<dyn Transform>) -> Self {
        Self { transform, name }
    }
}

#[async_trait]
impl Stage for TransformStageAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        // Get the first input (transforms typically work on a single input)
        let input_data = inputs
            .values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Transform '{}' requires at least one input", self.name))?
            .clone();

        let config_opt = if config.is_empty() {
            None
        } else {
            Some(config.clone())
        };

        self.transform.apply(input_data, &config_opt).await
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        let config_opt = if config.is_empty() {
            None
        } else {
            Some(config.clone())
        };
        self.transform.validate_config(&config_opt).await
    }
}

/// Adapter to convert Sink into Stage
pub struct SinkStageAdapter {
    sink: Arc<dyn Sink>,
    name: String,
}

impl SinkStageAdapter {
    pub fn new(name: String, sink: Arc<dyn Sink>) -> Self {
        Self { sink, name }
    }
}

#[async_trait]
impl Stage for SinkStageAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        // Get the first input
        let input_data = inputs
            .values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Sink '{}' requires at least one input", self.name))?
            .clone();

        self.sink.write(input_data.clone(), config).await?;

        // Return the input data for potential chaining (though sinks don't typically produce output)
        Ok(input_data)
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        self.sink.validate_config(config).await
    }

    fn produces_output(&self) -> bool {
        false // Sinks don't produce output for downstream stages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::DataFormat;
    use polars::prelude::DataFrame;

    struct MockSource;

    #[async_trait]
    impl DataSource for MockSource {
        async fn name(&self) -> &str {
            "mock"
        }

        async fn read(&self, _config: &HashMap<String, toml::Value>) -> Result<DataFormat> {
            Ok(DataFormat::DataFrame(DataFrame::empty()))
        }

        async fn validate_config(&self, _config: &HashMap<String, toml::Value>) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_source_adapter() {
        let source = Arc::new(MockSource);
        let stage = SourceStageAdapter::new("test_source".to_string(), source);

        assert_eq!(stage.name(), "test_source");
        assert!(stage.produces_output());

        let inputs = HashMap::new();
        let config = HashMap::new();
        let result = stage.execute(inputs, &config).await;
        assert!(result.is_ok());
    }
}
