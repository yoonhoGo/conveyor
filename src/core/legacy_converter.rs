use crate::core::config::{DagPipelineConfig, PipelineConfig, StageConfig};

/// Convert legacy PipelineConfig to new DAG-based DagPipelineConfig
pub struct LegacyConfigConverter;

impl LegacyConfigConverter {
    /// Convert a legacy pipeline configuration to DAG format
    pub fn convert(legacy: PipelineConfig) -> DagPipelineConfig {
        let mut stages = Vec::new();
        let mut prev_stage_id: Option<String> = None;

        // Convert sources to stages
        for source in &legacy.sources {
            let stage_id = source.name.clone();
            let stage_type = format!("source.{}", source.source_type);

            stages.push(StageConfig {
                id: stage_id.clone(),
                stage_type,
                inputs: vec![], // Sources have no inputs
                config: source.config.clone(),
            });

            prev_stage_id = Some(stage_id);
        }

        // Convert transforms to stages
        for transform in &legacy.transforms {
            let stage_id = transform.name.clone();
            let stage_type = format!("transform.{}", transform.function);

            // Determine inputs
            let inputs = if let Some(input) = &transform.input {
                // Explicit input specified
                vec![input.clone()]
            } else if let Some(prev_id) = &prev_stage_id {
                // Use previous stage as input
                vec![prev_id.clone()]
            } else {
                // No previous stage - this shouldn't happen in valid config
                vec![]
            };

            stages.push(StageConfig {
                id: stage_id.clone(),
                stage_type,
                inputs,
                config: transform.config.clone().unwrap_or_default(),
            });

            prev_stage_id = Some(stage_id);
        }

        // Convert sinks to stages
        for sink in &legacy.sinks {
            let stage_id = sink.name.clone();
            let stage_type = format!("sink.{}", sink.sink_type);

            // Determine inputs
            let inputs = if let Some(input) = &sink.input {
                // Explicit input specified
                vec![input.clone()]
            } else if let Some(prev_id) = &prev_stage_id {
                // Use previous stage (last transform or source) as input
                vec![prev_id.clone()]
            } else {
                // No previous stage - shouldn't happen
                vec![]
            };

            stages.push(StageConfig {
                id: stage_id,
                stage_type,
                inputs,
                config: sink.config.clone(),
            });
        }

        DagPipelineConfig {
            pipeline: legacy.pipeline,
            global: legacy.global,
            stages,
            error_handling: legacy.error_handling,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_converter() {
        let legacy_toml = r#"
[pipeline]
name = "legacy-pipeline"
version = "1.0"

[[sources]]
name = "users"
type = "json"

[sources.config]
path = "users.json"

[[transforms]]
name = "filter_active"
function = "filter"

[transforms.config]
column = "status"
operator = "=="
value = "active"

[[sinks]]
name = "output"
type = "json"

[sinks.config]
path = "output.json"
"#;

        let legacy_config: PipelineConfig = toml::from_str(legacy_toml).unwrap();
        let dag_config = LegacyConfigConverter::convert(legacy_config);

        assert_eq!(dag_config.stages.len(), 3);

        // Check source stage
        assert_eq!(dag_config.stages[0].id, "users");
        assert_eq!(dag_config.stages[0].stage_type, "source.json");
        assert!(dag_config.stages[0].inputs.is_empty());

        // Check transform stage
        assert_eq!(dag_config.stages[1].id, "filter_active");
        assert_eq!(dag_config.stages[1].stage_type, "transform.filter");
        assert_eq!(dag_config.stages[1].inputs, vec!["users"]);

        // Check sink stage
        assert_eq!(dag_config.stages[2].id, "output");
        assert_eq!(dag_config.stages[2].stage_type, "sink.json");
        assert_eq!(dag_config.stages[2].inputs, vec!["filter_active"]);
    }

    #[test]
    fn test_legacy_converter_with_explicit_inputs() {
        let legacy_toml = r#"
[pipeline]
name = "legacy-pipeline"
version = "1.0"

[[sources]]
name = "source1"
type = "json"

[[sources]]
name = "source2"
type = "csv"

[[transforms]]
name = "transform1"
function = "filter"
input = "source2"

[[sinks]]
name = "sink1"
type = "json"
input = "transform1"
"#;

        let legacy_config: PipelineConfig = toml::from_str(legacy_toml).unwrap();
        let dag_config = LegacyConfigConverter::convert(legacy_config);

        // Check that explicit inputs are preserved
        assert_eq!(dag_config.stages[2].inputs, vec!["source2"]);
        assert_eq!(dag_config.stages[3].inputs, vec!["transform1"]);
    }
}
