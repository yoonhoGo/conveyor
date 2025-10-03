use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::Stream;
use tracing::{debug, info};

use crate::core::traits::{RecordBatch, StreamingDataSource};

/// Streaming stdin source that reads line-by-line
pub struct StdinStreamSource;

impl Default for StdinStreamSource {
    fn default() -> Self {
        Self::new()
    }
}

impl StdinStreamSource {
    pub fn new() -> Self {
        Self
    }

    /// Parse a single line based on format
    fn parse_line(line: String, format: &str) -> Result<HashMap<String, JsonValue>> {
        match format {
            "json" | "jsonl" => {
                let record: HashMap<String, JsonValue> = serde_json::from_str(&line)?;
                Ok(record)
            }
            "csv" => {
                // Simple CSV parsing - split by comma
                // In production, use a proper CSV parser
                let fields: Vec<&str> = line.split(',').collect();
                let mut record = HashMap::new();
                for (i, field) in fields.iter().enumerate() {
                    record.insert(format!("field_{}", i), JsonValue::String(field.to_string()));
                }
                Ok(record)
            }
            "text" => {
                let mut record = HashMap::new();
                record.insert("line".to_string(), JsonValue::String(line));
                Ok(record)
            }
            _ => {
                anyhow::bail!("Unsupported format: {}", format)
            }
        }
    }
}

#[async_trait]
impl StreamingDataSource for StdinStreamSource {
    async fn name(&self) -> &str {
        "stdin_stream"
    }

    async fn stream(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>> {
        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("jsonl");

        info!("Starting stdin stream with format: {}", format);

        let format_owned = format.to_string();

        // Create async stdin reader
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        // Create stream that reads lines and parses them
        let stream = async_stream::stream! {
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("Read line from stdin: {}", line);

                match StdinStreamSource::parse_line(line, &format_owned) {
                    Ok(record) => {
                        // Yield a batch with single record
                        yield Ok(vec![record]);
                    }
                    Err(e) => {
                        yield Err(anyhow::anyhow!("Failed to parse line: {}", e));
                    }
                }
            }

            info!("Stdin stream ended");
        };

        Ok(Box::pin(stream))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if let Some(format_value) = config.get("format") {
            let format = format_value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("format must be a string"))?;

            let valid_formats = ["json", "jsonl", "csv", "text"];
            if !valid_formats.contains(&format) {
                anyhow::bail!(
                    "Invalid format: {}. Must be one of: {:?}",
                    format,
                    valid_formats
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_json() {
        let line = r#"{"name": "Alice", "age": 30}"#.to_string();
        let result = StdinStreamSource::parse_line(line, "json").unwrap();

        assert_eq!(
            result.get("name").unwrap(),
            &JsonValue::String("Alice".to_string())
        );
        assert_eq!(result.get("age").unwrap(), &JsonValue::Number(30.into()));
    }

    #[test]
    fn test_parse_line_text() {
        let line = "Hello, World!".to_string();
        let result = StdinStreamSource::parse_line(line.clone(), "text").unwrap();

        assert_eq!(result.get("line").unwrap(), &JsonValue::String(line));
    }

    #[tokio::test]
    async fn test_validate_config() {
        let source = StdinStreamSource::new();

        let mut config = HashMap::new();
        config.insert(
            "format".to_string(),
            toml::Value::String("jsonl".to_string()),
        );

        assert!(source.validate_config(&config).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_invalid_format() {
        let source = StdinStreamSource::new();

        let mut config = HashMap::new();
        config.insert(
            "format".to_string(),
            toml::Value::String("invalid".to_string()),
        );

        assert!(source.validate_config(&config).await.is_err());
    }
}
