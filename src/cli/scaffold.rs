use anyhow::Result;
use dialoguer::{Input, Select, Confirm};
use std::path::PathBuf;

pub fn scaffold_pipeline(output: Option<PathBuf>, interactive: bool) -> Result<()> {
    let (name, version, description) = if interactive {
        // Interactive mode: prompt for details
        let name: String = Input::new()
            .with_prompt("Pipeline name")
            .default("my_pipeline".to_string())
            .interact_text()?;

        let version: String = Input::new()
            .with_prompt("Pipeline version")
            .default("1.0.0".to_string())
            .interact_text()?;

        let description: String = Input::new()
            .with_prompt("Pipeline description")
            .default("A data processing pipeline".to_string())
            .interact_text()?;

        (name, version, description)
    } else {
        // Non-interactive mode: use defaults
        ("my_pipeline".to_string(), "1.0.0".to_string(), "A data processing pipeline".to_string())
    };

    let template = format!(
        r#"# Conveyor ETL Pipeline Configuration (DAG-based)
# Pipeline: {}
# Version: {}
# Description: {}

[pipeline]
name = "{}"
version = "{}"
description = "{}"

[global]
log_level = "info"  # Options: trace, debug, info, warn, error
max_parallel_tasks = 4
timeout_seconds = 300
# plugins = []  # Load dynamic plugins: ["http", "mongodb"]

# ============================================
# PIPELINE STAGES (DAG-based)
# ============================================
#
# Each stage has:
# - id: Unique identifier
# - type: Stage type (source.*, transform.*, sink.*, stage.pipeline, plugin.*, wasm.*)
# - inputs: List of stage IDs this stage depends on
# - config: Stage-specific configuration
#
# Example stages:
#
# [[stages]]
# id = "load_data"
# type = "source.csv"
# inputs = []
#
# [stages.config]
# path = "data/input.csv"
# headers = true
# delimiter = ","
#
# [[stages]]
# id = "filter_data"
# type = "transform.filter"
# inputs = ["load_data"]
#
# [stages.config]
# column = "status"
# operator = "=="
# value = "active"
#
# [[stages]]
# id = "save_results"
# type = "sink.json"
# inputs = ["filter_data"]
#
# [stages.config]
# path = "output/results.json"
# format = "records"
# pretty = true

# Add your stages below:
# (Uncomment and modify the examples above, or use 'conveyor add-stage' command)

# ============================================
# ERROR HANDLING
# ============================================

[error_handling]
strategy = "stop"  # Options: stop, continue, retry
max_retries = 3
retry_delay_seconds = 5

# Dead Letter Queue for failed records
# [error_handling.dead_letter_queue]
# enabled = true
# path = "errors/"
"#,
        name, version, description,
        name, version, description
    );

    if let Some(path) = output {
        // Check if file exists
        if path.exists() {
            if interactive {
                let overwrite = Confirm::new()
                    .with_prompt(format!("File {:?} already exists. Overwrite?", path))
                    .default(false)
                    .interact()?;

                if !overwrite {
                    println!("✗ Cancelled");
                    return Ok(());
                }
            } else {
                anyhow::bail!("File {:?} already exists. Use --force to overwrite or run in interactive mode", path);
            }
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, template)?;
        println!("✓ Scaffolded pipeline configuration written to: {:?}", path);
        println!("\nNext steps:");
        println!("  1. Edit the configuration file to add your stages");
        println!("  2. Or use 'conveyor add-stage {:?}' to add stages interactively", path);
        println!("  3. Validate with 'conveyor validate -c {:?}'", path);
        println!("  4. Run with 'conveyor run -c {:?}'", path);
    } else {
        println!("{}", template);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_scaffold_non_interactive() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Remove the file first (we just want the path)
        drop(temp_file);

        let result = scaffold_pipeline(Some(path.clone()), false);
        assert!(result.is_ok());
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("[pipeline]"));
        assert!(content.contains("name = \"my_pipeline\""));
        assert!(content.contains("[[stages]]"));
    }
}
