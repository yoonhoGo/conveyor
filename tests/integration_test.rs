use anyhow::Result;
use std::fs;
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

    // Create pipeline configuration (DAG format)
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

[[stages]]
id = "csv_input"
type = "source.csv"
inputs = []

[stages.config]
path = "{}"
headers = true
delimiter = ","

[[stages]]
id = "filter_high_value"
type = "transform.filter"
inputs = ["csv_input"]

[stages.config]
column = "value"
operator = ">="
value = 150

[[stages]]
id = "json_output"
type = "sink.json"
inputs = ["filter_high_value"]

[stages.config]
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
    let _output_csv = temp_dir.path().join("output.csv");

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

[[stages]]
id = "test_source"
type = "source.csv"
inputs = []

[stages.config]
path = "test.csv"
headers = true

[[stages]]
id = "test_sink"
type = "sink.json"
inputs = ["test_source"]

[stages.config]
path = "output.json"
format = "records"

[error_handling]
strategy = "stop"
max_retries = 3
retry_delay_seconds = 5
"#;

    // Parse config using conveyor's config parser
    use conveyor::core::config::DagPipelineConfig;
    let config: DagPipelineConfig = toml::from_str(config_str)?;

    assert_eq!(config.pipeline.name, "test");
    assert_eq!(config.stages.len(), 2);

    Ok(())
}
