use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

use crate::core::strategy::ErrorStrategy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetadata {
    pub name: String,

    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_max_parallel_tasks")]
    pub max_parallel_tasks: usize,

    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,

    /// List of plugins to load (e.g., ["http", "mongodb"])
    #[serde(default)]
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ErrorHandlingConfig {
    #[serde(default, flatten)]
    pub strategy: ErrorStrategy,

    #[serde(default)]
    pub dead_letter_queue: Option<DeadLetterQueueConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterQueueConfig {
    pub enabled: bool,
    pub path: String,
}

// Default value functions
fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_max_parallel_tasks() -> usize {
    4
}

fn default_timeout_seconds() -> u64 {
    300
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            max_parallel_tasks: default_max_parallel_tasks(),
            timeout_seconds: default_timeout_seconds(),
            plugins: Vec::new(),
        }
    }
}

/// Stage configuration for DAG-based pipelines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageConfig {
    /// Unique identifier for this stage
    pub id: String,

    /// Stage type (e.g., "source.json", "transform.filter", "sink.csv")
    #[serde(rename = "type")]
    pub stage_type: String,

    /// List of stage IDs this stage depends on (receives input from)
    #[serde(default)]
    pub inputs: Vec<String>,

    /// Stage-specific configuration
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,
}

/// DAG-based pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagPipelineConfig {
    pub pipeline: PipelineMetadata,

    #[serde(default)]
    pub global: GlobalConfig,

    #[serde(default)]
    pub stages: Vec<StageConfig>,

    #[serde(default)]
    pub error_handling: ErrorHandlingConfig,
}

impl DagPipelineConfig {
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path).await?;
        Self::from_str(&content)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str) -> Result<Self> {
        let config: DagPipelineConfig = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        // Validate that the pipeline has at least one stage
        if self.stages.is_empty() {
            anyhow::bail!("Pipeline must have at least one stage");
        }

        // Validate that all stage IDs are unique
        let mut ids = std::collections::HashSet::new();
        for stage in &self.stages {
            if !ids.insert(&stage.id) {
                anyhow::bail!("Duplicate stage id: '{}'", stage.id);
            }
        }

        // Validate that all input references exist
        for stage in &self.stages {
            for input_id in &stage.inputs {
                if !ids.contains(input_id) {
                    anyhow::bail!(
                        "Stage '{}' references non-existent input stage '{}'",
                        stage.id,
                        input_id
                    );
                }
            }
        }

        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.global.log_level.as_str()) {
            anyhow::bail!(
                "Invalid log level: {}. Must be one of: {:?}",
                self.global.log_level,
                valid_log_levels
            );
        }

        Ok(())
    }
}
