use anyhow::Result;
use conveyor::core::config::DagPipelineConfig;
use conveyor::core::dag_builder::DagPipelineBuilder;
use conveyor::core::registry::ModuleRegistry;
use conveyor::core::stage::Stage;
use std::sync::Arc;

#[tokio::test]
async fn test_pipeline_stage_inline() -> Result<()> {
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Create a temporary JSON file for main pipeline
    let mut temp_file1 = NamedTempFile::new()?;
    writeln!(
        temp_file1,
        r#"[{{"id": 1, "name": "Alice"}}, {{"id": 2, "name": "Bob"}}]"#
    )?;
    let temp_path1 = temp_file1.path().to_string_lossy().to_string();

    // Create a temporary JSON file for sub-pipeline
    let mut temp_file2 = NamedTempFile::new()?;
    writeln!(
        temp_file2,
        r#"[{{"id": 3, "value": 100}}, {{"id": 4, "value": 200}}]"#
    )?;
    let temp_path2 = temp_file2.path().to_string_lossy().to_string();

    // Escape the path for inline TOML (replace backslashes with double backslashes on Windows)
    let escaped_path2 = temp_path2.replace("\\", "\\\\");

    let config_str = format!(
        r#"
[pipeline]
name = "test-pipeline-stage"
version = "1.0"

[[stages]]
id = "load_data"
function = "json.read"
inputs = []

[stages.config]
format = "records"
path = "{}"

[[stages]]
id = "sub_pipeline"
function = "stage.pipeline"
inputs = ["load_data"]

[stages.config]
inline = """
[[stages]]
id = "load_sub"
function = "json.read"
inputs = []

[stages.config]
format = "records"
path = "{}"

[[stages]]
id = "filter"
function = "filter.apply"
inputs = ["load_sub"]

[stages.config]
column = "id"
operator = ">"
value = 0

[[stages]]
id = "output_sub"
function = "stdout.write"
inputs = ["filter"]

[stages.config]
format = "json"
"""

[[stages]]
id = "output"
function = "stdout.write"
inputs = ["sub_pipeline"]

[stages.config]
format = "json"
"#,
        temp_path1, escaped_path2
    );

    let config = DagPipelineConfig::from_str(&config_str)?;
    let registry = Arc::new(ModuleRegistry::with_defaults().await?);
    let builder = DagPipelineBuilder::new(registry);
    let mut executor = builder.build(&config)?;

    // Should execute without errors
    executor.execute().await?;

    Ok(())
}

#[tokio::test]
async fn test_pipeline_stage_validation() -> Result<()> {
    use conveyor::modules::stages::PipelineStage;
    use std::collections::HashMap;

    let registry = Arc::new(ModuleRegistry::with_defaults().await?);
    let stage = PipelineStage::new(registry);

    // Test: missing both file and inline
    let mut config = HashMap::new();
    assert!(stage.validate_config(&config).await.is_err());

    // Test: with inline
    let inline_config = r#"
[[stages]]
id = "test"
function = "json.read"
inputs = []

[stages.config]
format = "records"
content = "[]"
"#;

    config.insert(
        "inline".to_string(),
        toml::Value::String(inline_config.to_string()),
    );
    assert!(stage.validate_config(&config).await.is_ok());

    // Test: both file and inline (should fail)
    config.insert(
        "file".to_string(),
        toml::Value::String("test.toml".to_string()),
    );
    assert!(stage.validate_config(&config).await.is_err());

    Ok(())
}

#[tokio::test]
async fn test_nested_pipeline() -> Result<()> {
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Create a temporary JSON file for main pipeline
    let mut temp_file1 = NamedTempFile::new()?;
    writeln!(
        temp_file1,
        r#"[{{"value": 10}}, {{"value": 20}}, {{"value": 30}}]"#
    )?;
    let temp_path1 = temp_file1.path().to_string_lossy().to_string();

    // Create a temporary JSON file for sub-pipeline
    let mut temp_file2 = NamedTempFile::new()?;
    writeln!(
        temp_file2,
        r#"[{{"value": 15}}, {{"value": 25}}, {{"value": 35}}]"#
    )?;
    let temp_path2 = temp_file2.path().to_string_lossy().to_string();
    let escaped_path2 = temp_path2.replace("\\", "\\\\");

    // Create a more complex nested pipeline
    let config_str = format!(
        r#"
[pipeline]
name = "nested-test"
version = "1.0"

[[stages]]
id = "load"
function = "json.read"
inputs = []

[stages.config]
format = "records"
path = "{}"

[[stages]]
id = "process"
function = "stage.pipeline"
inputs = ["load"]

[stages.config]
inline = """
[[stages]]
id = "load_sub"
function = "json.read"
inputs = []

[stages.config]
format = "records"
path = "{}"

[[stages]]
id = "filter_low"
function = "filter.apply"
inputs = ["load_sub"]

[stages.config]
column = "value"
operator = ">="
value = 15

[[stages]]
id = "display"
function = "stdout.write"
inputs = ["filter_low"]

[stages.config]
format = "json"
"""

[[stages]]
id = "final_output"
function = "stdout.write"
inputs = ["process"]

[stages.config]
format = "table"
"#,
        temp_path1, escaped_path2
    );

    let config = DagPipelineConfig::from_str(&config_str)?;
    let registry = Arc::new(ModuleRegistry::with_defaults().await?);
    let builder = DagPipelineBuilder::new(registry);
    let mut executor = builder.build(&config)?;

    executor.execute().await?;

    Ok(())
}
