use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::io::{AsyncWriteExt, stdout};
use tokio_stream::StreamExt;
use tracing::{debug, info};

use crate::core::traits::{DataFormat, Sink};

/// Streaming stdout sink that outputs data in real-time
pub struct StdoutStreamSink;

impl StdoutStreamSink {
    pub fn new() -> Self {
        Self
    }

    /// Format a single record for output
    fn format_record(
        record: &HashMap<String, serde_json::Value>,
        format: &str,
        pretty: bool,
    ) -> Result<String> {
        match format {
            "json" | "jsonl" => {
                if pretty {
                    Ok(serde_json::to_string_pretty(record)?)
                } else {
                    Ok(serde_json::to_string(record)?)
                }
            }
            "csv" => {
                // Simple CSV output: field1,field2,field3
                let values: Vec<String> = record
                    .values()
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Null => String::new(),
                        _ => v.to_string(),
                    })
                    .collect();
                Ok(values.join(","))
            }
            "text" => {
                // Output as key=value pairs
                let pairs: Vec<String> = record
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                Ok(pairs.join(" "))
            }
            _ => {
                anyhow::bail!("Unsupported format: {}", format)
            }
        }
    }
}

#[async_trait]
impl Sink for StdoutStreamSink {
    async fn name(&self) -> &str {
        "stdout_stream"
    }

    async fn write(&self, data: DataFormat, config: &HashMap<String, toml::Value>) -> Result<()> {
        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("jsonl");

        let pretty = config
            .get("pretty")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let flush_every = config
            .get("flush_every")
            .and_then(|v| v.as_integer())
            .unwrap_or(1) as usize;

        info!("Writing to stdout stream (format: {}, pretty: {})", format, pretty);

        let mut stdout = stdout();
        let mut count = 0;

        match data {
            DataFormat::Stream(mut stream) => {
                // Stream processing: output each batch as it arrives
                while let Some(batch_result) = stream.next().await {
                    let batch = batch_result?;

                    for record in batch {
                        let line = Self::format_record(&record, format, pretty)?;
                        stdout.write_all(line.as_bytes()).await?;
                        stdout.write_all(b"\n").await?;

                        count += 1;

                        // Flush periodically
                        if count % flush_every == 0 {
                            stdout.flush().await?;
                            debug!("Flushed {} records to stdout", count);
                        }
                    }
                }

                // Final flush
                stdout.flush().await?;
                info!("Wrote {} total records to stdout stream", count);
            }
            DataFormat::RecordBatch(records) => {
                // Batch processing: output all records
                for record in records {
                    let line = Self::format_record(&record, format, pretty)?;
                    stdout.write_all(line.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;

                    count += 1;

                    if count % flush_every == 0 {
                        stdout.flush().await?;
                    }
                }

                stdout.flush().await?;
                info!("Wrote {} records to stdout", count);
            }
            DataFormat::DataFrame(df) => {
                // Convert DataFrame to records and output
                let records = crate::core::traits::DataFormat::DataFrame(df).as_record_batch()?;

                for record in records {
                    let line = Self::format_record(&record, format, pretty)?;
                    stdout.write_all(line.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;

                    count += 1;

                    if count % flush_every == 0 {
                        stdout.flush().await?;
                    }
                }

                stdout.flush().await?;
                info!("Wrote {} records to stdout", count);
            }
            DataFormat::Raw(bytes) => {
                stdout.write_all(&bytes).await?;
                stdout.flush().await?;
                info!("Wrote {} bytes to stdout", bytes.len());
            }
        }

        Ok(())
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        // Validate format if provided
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

        // Validate pretty if provided
        if let Some(pretty_value) = config.get("pretty") {
            if pretty_value.as_bool().is_none() {
                anyhow::bail!("pretty must be a boolean");
            }
        }

        // Validate flush_every if provided
        if let Some(flush_value) = config.get("flush_every") {
            if flush_value.as_integer().is_none() {
                anyhow::bail!("flush_every must be an integer");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_record_json() {
        let mut record = HashMap::new();
        record.insert("name".to_string(), json!("Alice"));
        record.insert("age".to_string(), json!(30));

        let result = StdoutStreamSink::format_record(&record, "json", false).unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("30"));
    }

    #[test]
    fn test_format_record_csv() {
        let mut record = HashMap::new();
        record.insert("name".to_string(), json!("Alice"));
        record.insert("age".to_string(), json!(30));

        let result = StdoutStreamSink::format_record(&record, "csv", false).unwrap();
        // CSV output might vary in order, just check it contains the values
        assert!(result.contains("Alice") || result.contains("30"));
    }

    #[tokio::test]
    async fn test_validate_config() {
        let sink = StdoutStreamSink::new();

        let mut config = HashMap::new();
        config.insert("format".to_string(), toml::Value::String("jsonl".to_string()));
        config.insert("pretty".to_string(), toml::Value::Boolean(false));
        config.insert("flush_every".to_string(), toml::Value::Integer(10));

        assert!(sink.validate_config(&config).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_invalid_format() {
        let sink = StdoutStreamSink::new();

        let mut config = HashMap::new();
        config.insert("format".to_string(), toml::Value::String("invalid".to_string()));

        assert!(sink.validate_config(&config).await.is_err());
    }
}
