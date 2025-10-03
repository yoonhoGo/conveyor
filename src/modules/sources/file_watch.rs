use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;
use tokio_stream::Stream;
use tracing::{debug, info, warn};

use crate::core::traits::{RecordBatch, StreamingDataSource};

/// File watch source that monitors files for changes
/// This is a simple polling-based implementation
pub struct FileWatchSource;

impl FileWatchSource {
    pub fn new() -> Self {
        Self
    }

    /// Parse file content based on format
    async fn parse_file(path: &PathBuf, format: &str) -> Result<RecordBatch> {
        let content = fs::read_to_string(path).await?;

        match format {
            "jsonl" => {
                let mut records = Vec::new();
                for line in content.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let record: HashMap<String, JsonValue> = serde_json::from_str(line)?;
                    records.push(record);
                }
                Ok(records)
            }
            "json" => {
                let records: Vec<HashMap<String, JsonValue>> = serde_json::from_str(&content)?;
                Ok(records)
            }
            "text" => {
                let records: Vec<HashMap<String, JsonValue>> = content
                    .lines()
                    .map(|line| {
                        let mut record = HashMap::new();
                        record.insert("line".to_string(), JsonValue::String(line.to_string()));
                        record
                    })
                    .collect();
                Ok(records)
            }
            _ => {
                anyhow::bail!("Unsupported format: {}", format)
            }
        }
    }
}

#[async_trait]
impl StreamingDataSource for FileWatchSource {
    async fn name(&self) -> &str {
        "file_watch"
    }

    async fn stream(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>> {
        let path_str = config
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'path' configuration"))?;

        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("jsonl");

        let poll_interval_secs = config
            .get("poll_interval")
            .and_then(|v| v.as_integer())
            .unwrap_or(1) as u64;

        let path = PathBuf::from(path_str);
        let format_owned = format.to_string();
        let poll_interval = Duration::from_secs(poll_interval_secs);

        info!(
            "Starting file watch on {:?} (format: {}, poll_interval: {}s)",
            path, format, poll_interval_secs
        );

        // Track last modification time
        let mut last_modified = None;

        let stream = async_stream::stream! {
            loop {
                match fs::metadata(&path).await {
                    Ok(metadata) => {
                        if let Ok(modified) = metadata.modified() {
                            let should_read = match last_modified {
                                None => {
                                    // First read
                                    last_modified = Some(modified);
                                    true
                                }
                                Some(last_mod) => {
                                    if modified > last_mod {
                                        debug!("File {:?} modified, reading...", path);
                                        last_modified = Some(modified);
                                        true
                                    } else {
                                        false
                                    }
                                }
                            };

                            if should_read {
                                match Self::parse_file(&path, &format_owned).await {
                                    Ok(records) => {
                                        if !records.is_empty() {
                                            info!("Read {} records from {:?}", records.len(), path);
                                            yield Ok(records);
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse file {:?}: {}", path, e);
                                        yield Err(e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("File {:?} not found or inaccessible: {}", path, e);
                        // Don't yield error, just wait for file to appear
                    }
                }

                sleep(poll_interval).await;
            }
        };

        Ok(Box::pin(stream))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        // Validate path
        if !config.contains_key("path") {
            anyhow::bail!("Missing required 'path' configuration");
        }

        // Validate format if provided
        if let Some(format_value) = config.get("format") {
            let format = format_value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("format must be a string"))?;

            let valid_formats = ["json", "jsonl", "text"];
            if !valid_formats.contains(&format) {
                anyhow::bail!(
                    "Invalid format: {}. Must be one of: {:?}",
                    format,
                    valid_formats
                );
            }
        }

        // Validate poll_interval if provided
        if let Some(interval_value) = config.get("poll_interval") {
            if interval_value.as_integer().is_none() {
                anyhow::bail!("poll_interval must be an integer (seconds)");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_parse_file_jsonl() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"{"name": "Alice", "age": 30}
{"name": "Bob", "age": 25}"#;
        tokio::fs::write(temp_file.path(), content).await.unwrap();

        let records = FileWatchSource::parse_file(&temp_file.path().to_path_buf(), "jsonl")
            .await
            .unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].get("name").unwrap(), &JsonValue::String("Alice".to_string()));
    }

    #[tokio::test]
    async fn test_parse_file_text() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "Line 1\nLine 2\nLine 3";
        tokio::fs::write(temp_file.path(), content).await.unwrap();

        let records = FileWatchSource::parse_file(&temp_file.path().to_path_buf(), "text")
            .await
            .unwrap();

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].get("line").unwrap(), &JsonValue::String("Line 1".to_string()));
    }

    #[tokio::test]
    async fn test_validate_config() {
        let source = FileWatchSource::new();

        let mut config = HashMap::new();
        config.insert("path".to_string(), toml::Value::String("/tmp/test.json".to_string()));
        config.insert("format".to_string(), toml::Value::String("jsonl".to_string()));
        config.insert("poll_interval".to_string(), toml::Value::Integer(2));

        assert!(source.validate_config(&config).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_missing_path() {
        let source = FileWatchSource::new();
        let config = HashMap::new();

        assert!(source.validate_config(&config).await.is_err());
    }
}
