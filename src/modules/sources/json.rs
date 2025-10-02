use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

use crate::core::traits::{DataFormat, DataSource, RecordBatch};

pub struct JsonSource;

#[async_trait]
impl DataSource for JsonSource {
    async fn name(&self) -> &str {
        "json"
    }

    async fn read(&self, config: &HashMap<String, toml::Value>) -> Result<DataFormat> {
        let path = config
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("JSON source requires 'path' configuration"))?;

        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("records");

        let path_buf = PathBuf::from(path);

        if !path_buf.exists() {
            anyhow::bail!("JSON file not found: {}", path);
        }

        let content = fs::read_to_string(&path_buf).await?;

        match format {
            "records" | "jsonl" => {
                // Parse as newline-delimited JSON or array of records
                let records: RecordBatch = if format == "jsonl" {
                    // Parse each line as a separate JSON object
                    content
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .map(|line| serde_json::from_str(line))
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    // Parse as a JSON array
                    serde_json::from_str(&content)?
                };

                Ok(DataFormat::RecordBatch(records))
            }
            "dataframe" => {
                // Parse directly into DataFrame using Polars
                let cursor = std::io::Cursor::new(content.as_bytes());
                let df = JsonReader::new(cursor).finish()?;
                Ok(DataFormat::DataFrame(df))
            }
            _ => anyhow::bail!(
                "Unknown JSON format: {}. Use 'records', 'jsonl', or 'dataframe'",
                format
            ),
        }
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("path") {
            anyhow::bail!("JSON source requires 'path' configuration");
        }

        if let Some(format) = config.get("format") {
            if let Some(fmt_str) = format.as_str() {
                let valid_formats = ["records", "jsonl", "dataframe"];
                if !valid_formats.contains(&fmt_str) {
                    anyhow::bail!(
                        "Invalid JSON format: {}. Must be one of: {:?}",
                        fmt_str,
                        valid_formats
                    );
                }
            } else {
                anyhow::bail!("Format must be a string");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_json_source_read_records() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"[{{"id": 1, "name": "Alice"}}, {{"id": 2, "name": "Bob"}}]"#
        )
        .unwrap();

        let mut config = HashMap::new();
        config.insert(
            "path".to_string(),
            toml::Value::String(temp_file.path().to_string_lossy().to_string()),
        );
        config.insert(
            "format".to_string(),
            toml::Value::String("records".to_string()),
        );

        let source = JsonSource;
        let result = source.read(&config).await;
        assert!(result.is_ok());

        let data = result.unwrap();
        match data {
            DataFormat::RecordBatch(records) => {
                assert_eq!(records.len(), 2);
            }
            _ => panic!("Expected RecordBatch"),
        }
    }

    #[tokio::test]
    async fn test_json_source_validation() {
        let source = JsonSource;

        // Valid config
        let mut valid_config = HashMap::new();
        valid_config.insert(
            "path".to_string(),
            toml::Value::String("test.json".to_string()),
        );
        assert!(source.validate_config(&valid_config).await.is_ok());

        // Invalid config - missing path
        let invalid_config = HashMap::new();
        assert!(source.validate_config(&invalid_config).await.is_err());
    }
}
