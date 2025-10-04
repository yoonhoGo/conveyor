use anyhow::Result;
use conveyor::core::config::DagPipelineConfig;
use conveyor::core::pipeline::DagPipeline;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_dag_pipeline_basic() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("input.json");
    let output_path = temp_dir.path().join("output.json");

    // Create input file
    let input_data = r#"[
        {"id": 1, "name": "Alice", "status": "active"},
        {"id": 2, "name": "Bob", "status": "inactive"},
        {"id": 3, "name": "Charlie", "status": "active"}
    ]"#;
    fs::write(&input_path, input_data)?;

    // Create DAG pipeline config
    // Convert paths to use forward slashes for TOML compatibility on Windows
    let input_path_str = input_path.to_string_lossy().replace('\\', "/");
    let output_path_str = output_path.to_string_lossy().replace('\\', "/");

    let config_str = format!(
        r#"
[pipeline]
name = "test-dag-pipeline"
version = "1.0"

[[stages]]
id = "load_data"
function = "json.read"
inputs = []

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "filter_active"
function = "filter.apply"
inputs = ["load_data"]

[stages.config]
column = "status"
operator = "=="
value = "active"

[[stages]]
id = "save_data"
function = "json.write"
inputs = ["filter_active"]

[stages.config]
path = "{}"
format = "records"
"#,
        input_path_str, output_path_str
    );

    let config = DagPipelineConfig::from_str(&config_str)?;
    let pipeline = DagPipeline::new(config).await?;

    pipeline.validate()?;
    pipeline.execute().await?;

    // Verify output
    assert!(output_path.exists());
    let output_data = fs::read_to_string(&output_path)?;
    assert!(output_data.contains("Alice"));
    assert!(output_data.contains("Charlie"));
    assert!(!output_data.contains("Bob"));

    Ok(())
}

#[tokio::test]
async fn test_dag_pipeline_branching() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("input.json");
    let output1_path = temp_dir.path().join("output1.json");
    let output2_path = temp_dir.path().join("output2.json");

    // Create input file
    let input_data = r#"[
        {"id": 1, "value": 10},
        {"id": 2, "value": 20}
    ]"#;
    fs::write(&input_path, input_data)?;

    // Create DAG pipeline with branching (one source, two sinks)
    // Convert paths to use forward slashes for TOML compatibility on Windows
    let input_path_str = input_path.to_string_lossy().replace('\\', "/");
    let output1_path_str = output1_path.to_string_lossy().replace('\\', "/");
    let output2_path_str = output2_path.to_string_lossy().replace('\\', "/");

    let config_str = format!(
        r#"
[pipeline]
name = "branching-pipeline"
version = "1.0"

[[stages]]
id = "load_data"
function = "json.read"
inputs = []

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "save_copy1"
function = "json.write"
inputs = ["load_data"]

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "save_copy2"
function = "json.write"
inputs = ["load_data"]

[stages.config]
path = "{}"
format = "records"
"#,
        input_path_str, output1_path_str, output2_path_str
    );

    let config = DagPipelineConfig::from_str(&config_str)?;
    let pipeline = DagPipeline::new(config).await?;

    pipeline.validate()?;
    pipeline.execute().await?;

    // Verify both outputs exist
    assert!(output1_path.exists());
    assert!(output2_path.exists());

    Ok(())
}

#[tokio::test]
async fn test_dag_pipeline_cycle_detection() -> Result<()> {
    // Create a pipeline with a cycle
    let config_str = r#"
[pipeline]
name = "cycle-pipeline"
version = "1.0"

[[stages]]
id = "stage1"
function = "json.read"
inputs = ["stage2"]

[stages.config]
path = "test.json"

[[stages]]
id = "stage2"
function = "filter.apply"
inputs = ["stage1"]

[stages.config]
column = "test"
operator = "=="
value = "test"
"#;

    let config = DagPipelineConfig::from_str(config_str)?;
    let pipeline = DagPipeline::new(config).await;

    // Should fail due to cycle
    assert!(pipeline.is_err());

    Ok(())
}
