use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

use crate::core::strategy::ErrorStrategy;

/// Pipeline execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Traditional batch processing
    Batch,
    /// Stream processing with micro-batching
    Streaming,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::Batch
    }
}

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

    /// Execution mode: batch or streaming
    #[serde(default)]
    pub execution_mode: ExecutionMode,

    /// Batch size for micro-batching in streaming mode
    #[serde(default = "default_stream_batch_size")]
    pub stream_batch_size: usize,

    /// Checkpoint interval (number of records) for streaming mode
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval: usize,

    /// Global variables that can be referenced in stage configs
    /// Supports environment variable substitution: ${ENV_VAR}
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

fn default_stream_batch_size() -> usize {
    1000
}

fn default_checkpoint_interval() -> usize {
    5000
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            max_parallel_tasks: default_max_parallel_tasks(),
            timeout_seconds: default_timeout_seconds(),
            plugins: Vec::new(),
            execution_mode: ExecutionMode::default(),
            stream_batch_size: default_stream_batch_size(),
            checkpoint_interval: default_checkpoint_interval(),
            variables: HashMap::new(),
        }
    }
}

/// Stage configuration for DAG-based pipelines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageConfig {
    /// Unique identifier for this stage
    pub id: String,

    /// Function name (e.g., "csv.read", "mongodb.find", "filter.apply")
    pub function: String,

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
        let mut config: DagPipelineConfig = toml::from_str(content)?;

        // Resolve environment variables in global.variables
        config.resolve_variables()?;

        // Interpolate variables in stage configs
        config.interpolate_stage_configs()?;

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

    /// Substitute environment variables in global variables
    /// Replaces ${ENV_VAR} patterns with actual environment variable values
    pub fn resolve_variables(&mut self) -> Result<()> {
        use regex::Regex;
        use std::env;

        let env_var_regex = Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").unwrap();
        let mut resolved_variables = HashMap::new();

        for (key, value) in &self.global.variables {
            let mut resolved_value = value.clone();

            // Replace all ${ENV_VAR} patterns
            for cap in env_var_regex.captures_iter(value) {
                let env_var_name = &cap[1];
                let env_value = env::var(env_var_name).map_err(|_| {
                    anyhow::anyhow!(
                        "Environment variable '{}' referenced in variable '{}' is not set",
                        env_var_name,
                        key
                    )
                })?;
                resolved_value = resolved_value.replace(&cap[0], &env_value);
            }

            resolved_variables.insert(key.clone(), resolved_value);
        }

        self.global.variables = resolved_variables;
        Ok(())
    }

    /// Interpolate variables in a string value
    /// Replaces {{var_name}} patterns with values from global.variables
    pub fn interpolate_value(&self, value: &str) -> Result<String> {
        use regex::Regex;

        let var_regex = Regex::new(r"\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}").unwrap();
        let mut result = value.to_string();

        for cap in var_regex.captures_iter(value) {
            let var_name = &cap[1];
            let var_value = self.global.variables.get(var_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "Variable '{}' referenced but not defined in global.variables",
                    var_name
                )
            })?;
            result = result.replace(&cap[0], var_value);
        }

        Ok(result)
    }

    /// Interpolate variables in all stage configurations
    pub fn interpolate_stage_configs(&mut self) -> Result<()> {
        use regex::Regex;

        let var_regex = Regex::new(r"\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}").unwrap();

        // Clone variables to avoid borrow checker issues
        let variables = self.global.variables.clone();

        for stage in &mut self.stages {
            let mut new_config = HashMap::new();

            for (key, value) in &stage.config {
                let interpolated_value = match value {
                    toml::Value::String(s) => {
                        let mut result = s.clone();

                        for cap in var_regex.captures_iter(s) {
                            let var_name = &cap[1];
                            let var_value = variables.get(var_name).ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Variable '{}' referenced but not defined in global.variables",
                                    var_name
                                )
                            })?;
                            result = result.replace(&cap[0], var_value);
                        }

                        toml::Value::String(result)
                    }
                    other => other.clone(),
                };
                new_config.insert(key.clone(), interpolated_value);
            }

            stage.config = new_config;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_interpolation() {
        let toml_str = r#"
[pipeline]
name = "test"
version = "1.0"

[global]
log_level = "info"

[global.variables]
api_key = "secret-key-123"
base_url = "https://api.example.com"

[[stages]]
id = "stage1"
function = "test"

[stages.config]
url = "{{base_url}}/endpoint"
key = "{{api_key}}"
        "#;

        let config = DagPipelineConfig::from_str(toml_str).unwrap();

        assert_eq!(
            config.stages[0]
                .config
                .get("url")
                .unwrap()
                .as_str()
                .unwrap(),
            "https://api.example.com/endpoint"
        );
        assert_eq!(
            config.stages[0]
                .config
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            "secret-key-123"
        );
    }

    #[test]
    fn test_env_var_substitution() {
        std::env::set_var("TEST_API_KEY", "env-secret-123");
        std::env::set_var("TEST_BASE_URL", "https://env.example.com");

        let toml_str = r#"
[pipeline]
name = "test"
version = "1.0"

[global]
log_level = "info"

[global.variables]
api_key = "${TEST_API_KEY}"
base_url = "${TEST_BASE_URL}"

[[stages]]
id = "stage1"
function = "test"

[stages.config]
url = "{{base_url}}/endpoint"
key = "{{api_key}}"
        "#;

        let config = DagPipelineConfig::from_str(toml_str).unwrap();

        assert_eq!(
            config.global.variables.get("api_key").unwrap(),
            "env-secret-123"
        );
        assert_eq!(
            config.global.variables.get("base_url").unwrap(),
            "https://env.example.com"
        );
        assert_eq!(
            config.stages[0]
                .config
                .get("url")
                .unwrap()
                .as_str()
                .unwrap(),
            "https://env.example.com/endpoint"
        );
        assert_eq!(
            config.stages[0]
                .config
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            "env-secret-123"
        );

        std::env::remove_var("TEST_API_KEY");
        std::env::remove_var("TEST_BASE_URL");
    }

    #[test]
    fn test_missing_variable_error() {
        let toml_str = r#"
[pipeline]
name = "test"
version = "1.0"

[global]
log_level = "info"

[global.variables]
api_key = "secret-key-123"

[[stages]]
id = "stage1"
function = "test"

[stages.config]
url = "{{base_url}}/endpoint"
        "#;

        let result = DagPipelineConfig::from_str(toml_str);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Variable 'base_url' referenced but not defined"));
    }

    #[test]
    fn test_missing_env_var_error() {
        std::env::remove_var("NONEXISTENT_VAR");

        let toml_str = r#"
[pipeline]
name = "test"
version = "1.0"

[global]
log_level = "info"

[global.variables]
api_key = "${NONEXISTENT_VAR}"

[[stages]]
id = "stage1"
function = "test"
        "#;

        let result = DagPipelineConfig::from_str(toml_str);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Environment variable 'NONEXISTENT_VAR'"));
    }

    #[test]
    fn test_mixed_env_and_literal() {
        std::env::set_var("TEST_SECRET", "my-secret");

        let toml_str = r#"
[pipeline]
name = "test"
version = "1.0"

[global]
log_level = "info"

[global.variables]
token = "Bearer ${TEST_SECRET}"
base_url = "https://api.example.com"

[[stages]]
id = "stage1"
function = "test"

[stages.config]
auth_header = "{{token}}"
url = "{{base_url}}"
        "#;

        let config = DagPipelineConfig::from_str(toml_str).unwrap();

        assert_eq!(
            config.global.variables.get("token").unwrap(),
            "Bearer my-secret"
        );
        assert_eq!(
            config.stages[0]
                .config
                .get("auth_header")
                .unwrap()
                .as_str()
                .unwrap(),
            "Bearer my-secret"
        );

        std::env::remove_var("TEST_SECRET");
    }
}
