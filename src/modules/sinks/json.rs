use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::core::traits::{DataFormat, Sink};

pub struct JsonSink;

#[async_trait]
impl Sink for JsonSink {
    async fn name(&self) -> &str {
        "json"
    }

    async fn write(
        &self,
        data: DataFormat,
        config: &HashMap<String, toml::Value>,
    ) -> Result<()> {
        let path = config
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("JSON sink requires 'path' configuration"))?;

        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("records");

        let pretty = config
            .get("pretty")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path_buf = PathBuf::from(path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = path_buf.parent() {
            fs::create_dir_all(parent).await?;
        }

        let output = match format {
            "records" => {
                let records = data.as_record_batch()?;
                if pretty {
                    serde_json::to_string_pretty(&records)?
                } else {
                    serde_json::to_string(&records)?
                }
            }
            "jsonl" => {
                let records = data.as_record_batch()?;
                records
                    .iter()
                    .map(|record| serde_json::to_string(record))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\n")
            }
            "dataframe" => {
                let df = data.as_dataframe()?;
                let mut result = HashMap::new();

                for column in df.get_columns() {
                    let name = column.name().to_string();
                    let values: Vec<JsonValue> = match column.dtype() {
                        DataType::String => {
                            let s = column.str()?;
                            (0..column.len())
                                .map(|i| {
                                    s.get(i)
                                        .map(|v| JsonValue::String(v.to_string()))
                                        .unwrap_or(JsonValue::Null)
                                })
                                .collect()
                        }
                        DataType::Int64 => {
                            let s = column.i64()?;
                            (0..column.len())
                                .map(|i| {
                                    s.get(i)
                                        .map(|v| json!(v))
                                        .unwrap_or(JsonValue::Null)
                                })
                                .collect()
                        }
                        DataType::Float64 => {
                            let s = column.f64()?;
                            (0..column.len())
                                .map(|i| {
                                    s.get(i)
                                        .and_then(|v| serde_json::Number::from_f64(v).map(JsonValue::Number))
                                        .unwrap_or(JsonValue::Null)
                                })
                                .collect()
                        }
                        DataType::Boolean => {
                            let s = column.bool()?;
                            (0..column.len())
                                .map(|i| {
                                    s.get(i)
                                        .map(JsonValue::Bool)
                                        .unwrap_or(JsonValue::Null)
                                })
                                .collect()
                        }
                        _ => vec![JsonValue::Null; column.len()],
                    };
                    result.insert(name, values);
                }

                if pretty {
                    serde_json::to_string_pretty(&result)?
                } else {
                    serde_json::to_string(&result)?
                }
            }
            _ => anyhow::bail!("Unknown JSON format: {}. Use 'records', 'jsonl', or 'dataframe'", format),
        };

        let mut file = fs::File::create(&path_buf).await?;
        file.write_all(output.as_bytes()).await?;

        let row_count = match &data {
            DataFormat::DataFrame(df) => df.height(),
            DataFormat::RecordBatch(records) => records.len(),
            _ => 0,
        };

        tracing::info!("Written {} rows to JSON file: {}", row_count, path);

        Ok(())
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("path") {
            anyhow::bail!("JSON sink requires 'path' configuration");
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