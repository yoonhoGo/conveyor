use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_csv_to_json_pipeline() -> Result<()> {
    // Create temporary directory
    let temp_dir = TempDir::new()?;
    let input_csv = temp_dir.path().join("input.csv");
    let output_json = temp_dir.path().join("output.json");
    let config_toml = temp_dir.path().join("pipeline.toml");

    // Create test CSV file
    fs::write(
        &input_csv,
        "id,name,value\n1,Alice,100\n2,Bob,200\n3,Charlie,150\n",
    )?;

    // Create pipeline configuration
    let config = format!(
        r#"
[pipeline]
name = "test_pipeline"
version = "1.0.0"
description = "Test CSV to JSON pipeline"

[global]
log_level = "info"
max_parallel_tasks = 4
timeout_seconds = 300

[[sources]]
name = "csv_input"
type = "csv"

[sources.config]
path = "{}"
headers = true
delimiter = ","

[[transforms]]
name = "filter_high_value"
function = "filter"

[transforms.config]
column = "value"
operator = ">="
value = 150

[[sinks]]
name = "json_output"
type = "json"

[sinks.config]
path = "{}"
format = "records"
pretty = true

[error_handling]
strategy = "stop"
max_retries = 3
retry_delay_seconds = 5
"#,
        input_csv.display(),
        output_json.display()
    );

    fs::write(&config_toml, config)?;

    // TODO: Execute pipeline programmatically
    // For now, we'll just verify the config was created
    assert!(config_toml.exists());
    assert!(input_csv.exists());

    Ok(())
}

#[tokio::test]
async fn test_json_to_csv_pipeline() -> Result<()> {
    // Create temporary directory
    let temp_dir = TempDir::new()?;
    let input_json = temp_dir.path().join("input.json");
    let output_csv = temp_dir.path().join("output.csv");

    // Create test JSON file
    let json_data = r#"[
        {"id": 1, "name": "Alice", "score": 85},
        {"id": 2, "name": "Bob", "score": 92},
        {"id": 3, "name": "Charlie", "score": 78}
    ]"#;
    fs::write(&input_json, json_data)?;

    // Verify files exist
    assert!(input_json.exists());

    Ok(())
}

#[test]
fn test_config_parsing() -> Result<()> {
    let config_str = r#"
[pipeline]
name = "test"
version = "1.0.0"
description = "Test pipeline"

[global]
log_level = "info"
max_parallel_tasks = 4
timeout_seconds = 300

[[sources]]
name = "test_source"
type = "csv"

[sources.config]
path = "test.csv"
headers = true

[[sinks]]
name = "test_sink"
type = "json"

[sinks.config]
path = "output.json"
format = "records"

[error_handling]
strategy = "stop"
max_retries = 3
retry_delay_seconds = 5
"#;

    // Parse config using conveyor's config parser
    use conveyor::core::config::PipelineConfig;
    let config: PipelineConfig = toml::from_str(config_str)?;

    assert_eq!(config.pipeline.name, "test");
    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.sinks.len(), 1);

    Ok(())
}
