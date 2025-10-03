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
    let config_str = format!(
        r#"
[pipeline]
name = "test-dag-pipeline"
version = "1.0"

[[stages]]
id = "load_data"
type = "source.json"
inputs = []

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "filter_active"
type = "transform.filter"
inputs = ["load_data"]

[stages.config]
column = "status"
operator = "=="
value = "active"

[[stages]]
id = "save_data"
type = "sink.json"
inputs = ["filter_active"]

[stages.config]
path = "{}"
format = "records"
"#,
        input_path.display(),
        output_path.display()
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
    let config_str = format!(
        r#"
[pipeline]
name = "branching-pipeline"
version = "1.0"

[[stages]]
id = "load_data"
type = "source.json"
inputs = []

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "save_copy1"
type = "sink.json"
inputs = ["load_data"]

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "save_copy2"
type = "sink.json"
inputs = ["load_data"]

[stages.config]
path = "{}"
format = "records"
"#,
        input_path.display(),
        output1_path.display(),
        output2_path.display()
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
type = "source.json"
inputs = ["stage2"]

[stages.config]
path = "test.json"

[[stages]]
id = "stage2"
type = "transform.filter"
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

#[tokio::test]
#[ignore] // Legacy format auto-conversion not yet implemented
async fn test_legacy_to_dag_conversion() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("input.csv");
    let output_path = temp_dir.path().join("output.json");

    // Create input CSV
    let input_data = "id,name,status\n1,Alice,active\n2,Bob,inactive\n3,Charlie,active";
    fs::write(&input_path, input_data)?;

    // Create legacy format config
    let config_str = format!(
        r#"
[pipeline]
name = "legacy-pipeline"
version = "1.0"

[[sources]]
name = "load_data"
type = "csv"

[sources.config]
path = "{}"
has_header = true

[[transforms]]
name = "filter_active"
function = "filter"

[transforms.config]
column = "status"
operator = "=="
value = "active"

[[sinks]]
name = "save_data"
type = "json"

[sinks.config]
path = "{}"
format = "records"
"#,
        input_path.display(),
        output_path.display()
    );

    // First save the config
    fs::write(temp_dir.path().join("pipeline.toml"), config_str)?;

    // Use DagPipeline which should auto-convert legacy format
    let pipeline = DagPipeline::from_file(temp_dir.path().join("pipeline.toml")).await?;

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
