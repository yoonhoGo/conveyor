use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct StdoutSink;

#[async_trait]
impl Stage for StdoutSink {
    fn name(&self) -> &str {
        "stdout.write"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert(
            "format".to_string(),
            toml::Value::String("table".to_string()),
        );

        let mut example2 = HashMap::new();
        example2.insert(
            "format".to_string(),
            toml::Value::String("json".to_string()),
        );
        example2.insert("pretty".to_string(), toml::Value::Boolean(true));
        example2.insert("limit".to_string(), toml::Value::Integer(10));

        StageMetadata::builder("stdout.write", StageCategory::Sink)
            .description("Write data to standard output")
            .long_description(
                "Outputs data to stdout in various formats for display or piping to other commands. \
                Supports table, JSON, JSON Lines (jsonl), and CSV formats. \
                Can limit output rows for preview purposes. \
                Ideal for debugging pipelines or integrating with Unix tools."
            )
            .parameter(ConfigParameter::optional(
                "format",
                ParameterType::String,
                "table",
                "Output format"
            ).with_validation(ParameterValidation::allowed_values([
                "table", "json", "jsonl", "csv"
            ])))
            .parameter(ConfigParameter::optional(
                "limit",
                ParameterType::Integer,
                "unlimited",
                "Maximum number of rows to output (useful for preview)"
            ))
            .parameter(ConfigParameter::optional(
                "pretty",
                ParameterType::Boolean,
                "true",
                "Pretty-print JSON output (only applies to 'json' format)"
            ))
            .parameter(ConfigParameter::optional(
                "delimiter",
                ParameterType::String,
                ",",
                "Field delimiter for CSV format (only applies to 'csv' format)"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Table format output",
                example1,
                Some("Display data as formatted table")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "JSON preview",
                example2,
                Some("Output first 10 rows as pretty JSON")
            ))
            .tag("stdout")
            .tag("output")
            .tag("display")
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
            .ok_or_else(|| anyhow::anyhow!("Stdout sink requires input data"))?;
        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("table");

        let limit = config
            .get("limit")
            .and_then(|v| v.as_integer())
            .map(|v| v as usize);

        match format {
            "table" => {
                let df = data.as_dataframe()?;
                if let Some(limit) = limit {
                    println!("{}", df.head(Some(limit)));
                } else {
                    println!("{}", df);
                }
            }
            "json" => {
                let records = data.as_record_batch()?;
                let to_print = if let Some(limit) = limit {
                    &records[..limit.min(records.len())]
                } else {
                    &records[..]
                };

                let pretty = config
                    .get("pretty")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                if pretty {
                    println!("{}", serde_json::to_string_pretty(to_print)?);
                } else {
                    println!("{}", serde_json::to_string(to_print)?);
                }
            }
            "jsonl" => {
                let records = data.as_record_batch()?;
                let to_print = if let Some(limit) = limit {
                    &records[..limit.min(records.len())]
                } else {
                    &records[..]
                };

                for record in to_print {
                    println!("{}", serde_json::to_string(record)?);
                }
            }
            "csv" => {
                let df = data.as_dataframe()?;
                let mut df_to_print = if let Some(limit) = limit {
                    df.head(Some(limit))
                } else {
                    df.clone()
                };

                let delimiter = config
                    .get("delimiter")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.chars().next())
                    .unwrap_or(',') as u8;

                let mut buffer = Vec::new();
                CsvWriter::new(&mut buffer)
                    .include_header(true)
                    .with_separator(delimiter)
                    .finish(&mut df_to_print)?;

                print!("{}", String::from_utf8(buffer)?);
            }
            _ => anyhow::bail!(
                "Unknown stdout format: {}. Use 'table', 'json', 'jsonl', or 'csv'",
                format
            ),
        }

        Ok(DataFormat::RecordBatch(vec![]))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if let Some(format) = config.get("format") {
            if let Some(fmt_str) = format.as_str() {
                let valid_formats = ["table", "json", "jsonl", "csv"];
                if !valid_formats.contains(&fmt_str) {
                    anyhow::bail!(
                        "Invalid stdout format: {}. Must be one of: {:?}",
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
