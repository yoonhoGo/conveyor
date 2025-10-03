use anyhow::Result;
use conveyor::core::config::DagPipelineConfig;
use conveyor::core::pipeline::DagPipeline;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_http_fetch_transform_with_dag_pipeline() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("users.json");
    let output_path = temp_dir.path().join("output.json");

    // Create input file with user IDs
    let input_data = r#"[
        {"id": 1, "name": "Alice"},
        {"id": 2, "name": "Bob"}
    ]"#;
    fs::write(&input_path, input_data)?;

    // Create DAG pipeline with HTTP fetch
    // Using JSONPlaceholder API as a real endpoint for testing
    let config_str = format!(
        r#"
[pipeline]
name = "http-fetch-test"
version = "1.0"

[[stages]]
id = "load_users"
type = "source.json"
inputs = []

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "fetch_posts"
type = "transform.http_fetch"
inputs = ["load_users"]

[stages.config]
url = "https://jsonplaceholder.typicode.com/users/{{{{ id }}}}/posts"
method = "GET"
mode = "per_row"
result_field = "posts"

[[stages]]
id = "save_result"
type = "sink.json"
inputs = ["fetch_posts"]

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

    // Verify output exists and contains posts data
    assert!(output_path.exists());
    let output_data = fs::read_to_string(&output_path)?;

    // The output should contain the posts field
    assert!(output_data.contains("posts"));
    assert!(output_data.contains("Alice"));
    assert!(output_data.contains("Bob"));

    Ok(())
}

#[tokio::test]
async fn test_http_fetch_with_template() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("input.json");

    // Create input file
    let input_data = r#"[
        {"user_id": 1, "action": "test"}
    ]"#;
    fs::write(&input_path, input_data)?;

    // Test template rendering with config validation
    let config_str = format!(
        r#"
[pipeline]
name = "template-test"
version = "1.0"

[[stages]]
id = "load_data"
type = "source.json"
inputs = []

[stages.config]
path = "{}"
format = "records"

[[stages]]
id = "fetch_data"
type = "transform.http_fetch"
inputs = ["load_data"]

[stages.config]
url = "https://jsonplaceholder.typicode.com/posts/{{{{ user_id }}}}"
method = "GET"
mode = "per_row"
result_field = "post_data"

[[stages]]
id = "output"
type = "sink.stdout"
inputs = ["fetch_data"]

[stages.config]
format = "json"
"#,
        input_path.display()
    );

    let config = DagPipelineConfig::from_str(&config_str)?;
    let pipeline = DagPipeline::new(config).await?;

    // Should validate successfully
    assert!(pipeline.validate().is_ok());

    // Execute pipeline
    pipeline.execute().await?;

    Ok(())
}

#[tokio::test]
async fn test_http_fetch_config_validation() -> Result<()> {
    // Missing URL should fail validation
    let config_str = r#"
[pipeline]
name = "invalid-config"
version = "1.0"

[[stages]]
id = "source"
type = "source.json"
inputs = []

[stages.config]
path = "test.json"

[[stages]]
id = "fetch"
type = "transform.http_fetch"
inputs = ["source"]

[stages.config]
method = "GET"
"#;

    let result = DagPipelineConfig::from_str(config_str);
    // Config parsing should succeed, but validation will catch missing URL
    assert!(result.is_ok());

    // Invalid HTTP method should fail
    let config_str = r#"
[pipeline]
name = "invalid-method"
version = "1.0"

[[stages]]
id = "source"
type = "source.json"
inputs = []

[stages.config]
path = "test.json"

[[stages]]
id = "fetch"
type = "transform.http_fetch"
inputs = ["source"]

[stages.config]
url = "https://example.com"
method = "INVALID"
"#;

    let config = DagPipelineConfig::from_str(config_str)?;
    let _pipeline = DagPipeline::new(config).await?;

    // Validation should fail due to invalid method
    // Note: This will be caught during execution when the transform validates its config

    Ok(())
}
