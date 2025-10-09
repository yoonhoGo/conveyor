use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::core::metadata::{ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct JsonSink;

#[async_trait]
impl Stage for JsonSink {
    fn name(&self) -> &str {
        "json.write"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert("path".to_string(), toml::Value::String("output.json".to_string()));
        example1.insert("format".to_string(), toml::Value::String("records".to_string()));

        let mut example2 = HashMap::new();
        example2.insert("path".to_string(), toml::Value::String("output.jsonl".to_string()));
        example2.insert("format".to_string(), toml::Value::String("jsonl".to_string()));

        StageMetadata::builder("json.write", StageCategory::Sink)
            .description("Write data to JSON files")
            .long_description(
                "Writes DataFrame to JSON files in various formats. \
                Supports standard JSON array (records), JSON Lines (jsonl), and nested DataFrame format. \
                Can optionally pretty-print the output for better readability. \
                Automatically creates parent directories if they don't exist."
            )
            .parameter(ConfigParameter::required(
                "path",
                ParameterType::String,
                "Path to the output JSON file"
            ))
            .parameter(ConfigParameter::optional(
                "format",
                ParameterType::String,
                "records",
                "JSON output format"
            ).with_validation(ParameterValidation::allowed_values([
                "records", "jsonl", "dataframe"
            ])))
            .parameter(ConfigParameter::optional(
                "pretty",
                ParameterType::Boolean,
                "false",
                "Pretty-print the JSON output (not applicable to jsonl format)"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Standard JSON output",
                example1,
                Some("Write data as JSON array of objects")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "JSON Lines output",
                example2,
                Some("Write data in JSON Lines format (one object per line)")
            ))
            .tag("json")
            .tag("file")
            .tag("io")
            .tag("sink")
            .build()
    }

    fn produces_output(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("JSON sink requires input data"))?;
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
                    .map(serde_json::to_string)
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
                                .map(|i| s.get(i).map(|v| json!(v)).unwrap_or(JsonValue::Null))
                                .collect()
                        }
                        DataType::Float64 => {
                            let s = column.f64()?;
                            (0..column.len())
                                .map(|i| {
                                    s.get(i)
                                        .and_then(|v| {
                                            serde_json::Number::from_f64(v).map(JsonValue::Number)
                                        })
                                        .unwrap_or(JsonValue::Null)
                                })
                                .collect()
                        }
                        DataType::Boolean => {
                            let s = column.bool()?;
                            (0..column.len())
                                .map(|i| s.get(i).map(JsonValue::Bool).unwrap_or(JsonValue::Null))
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
            _ => anyhow::bail!(
                "Unknown JSON format: {}. Use 'records', 'jsonl', or 'dataframe'",
                format
            ),
        };

        let mut file = fs::File::create(&path_buf).await?;
        file.write_all(output.as_bytes()).await?;

        let row_count = match &data {
            DataFormat::DataFrame(df) => df.height(),
            DataFormat::RecordBatch(records) => records.len(),
            _ => 0,
        };

        tracing::info!("Written {} rows to JSON file: {}", row_count, path);

        Ok(DataFormat::RecordBatch(vec![]))
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
