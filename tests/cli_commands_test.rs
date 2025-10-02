use anyhow::Result;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_scaffold_creates_valid_pipeline() -> Result<()> {
    use conveyor::cli::scaffold_pipeline;

    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path().to_path_buf();
    drop(temp_file);

    // Test non-interactive mode
    scaffold_pipeline(Some(path.clone()), false)?;

    // Check file exists
    assert!(path.exists());

    // Check file contains expected content
    let content = fs::read_to_string(&path)?;
    assert!(content.contains("[pipeline]"));
    assert!(content.contains("name = \"my_pipeline\""));
    assert!(content.contains("version = \"1.0.0\""));
    assert!(content.contains("[global]"));
    assert!(content.contains("[error_handling]"));
    assert!(content.contains("# [[stages]]"));

    // Cleanup
    fs::remove_file(path)?;

    Ok(())
}

#[test]
fn test_scaffold_output_to_stdout() -> Result<()> {
    use conveyor::cli::scaffold_pipeline;

    // Test output to stdout (should not error)
    let result = scaffold_pipeline(None, false);
    assert!(result.is_ok());

    Ok(())
}

#[test]
fn test_scaffold_creates_valid_toml() -> Result<()> {
    use conveyor::cli::scaffold_pipeline;

    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path().to_path_buf();
    drop(temp_file);

    scaffold_pipeline(Some(path.clone()), false)?;

    // Try to parse as TOML
    let content = fs::read_to_string(&path)?;
    let parsed: toml::Value = toml::from_str(&content)?;

    // Verify structure
    assert!(parsed.get("pipeline").is_some());
    assert!(parsed.get("global").is_some());
    assert!(parsed.get("error_handling").is_some());

    // Cleanup
    fs::remove_file(path)?;

    Ok(())
}

#[test]
fn test_scaffold_respects_output_path() -> Result<()> {
    use conveyor::cli::scaffold_pipeline;
    use std::env::temp_dir;

    let custom_path = temp_dir().join("custom_pipeline.toml");

    // Remove if exists
    let _ = fs::remove_file(&custom_path);

    scaffold_pipeline(Some(custom_path.clone()), false)?;

    assert!(custom_path.exists());

    // Cleanup
    fs::remove_file(custom_path)?;

    Ok(())
}

#[test]
fn test_generated_pipeline_has_correct_metadata() -> Result<()> {
    use conveyor::cli::scaffold_pipeline;

    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path().to_path_buf();
    drop(temp_file);

    scaffold_pipeline(Some(path.clone()), false)?;

    let content = fs::read_to_string(&path)?;
    let parsed: toml::Value = toml::from_str(&content)?;

    // Check pipeline metadata
    let pipeline = parsed.get("pipeline").unwrap();
    assert_eq!(
        pipeline.get("name").and_then(|v| v.as_str()),
        Some("my_pipeline")
    );
    assert_eq!(
        pipeline.get("version").and_then(|v| v.as_str()),
        Some("1.0.0")
    );
    assert_eq!(
        pipeline.get("description").and_then(|v| v.as_str()),
        Some("A data processing pipeline")
    );

    // Check global config
    let global = parsed.get("global").unwrap();
    assert_eq!(
        global.get("log_level").and_then(|v| v.as_str()),
        Some("info")
    );
    assert_eq!(
        global
            .get("max_parallel_tasks")
            .and_then(|v| v.as_integer()),
        Some(4)
    );

    // Cleanup
    fs::remove_file(path)?;

    Ok(())
}
