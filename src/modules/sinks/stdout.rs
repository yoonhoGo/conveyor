use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::traits::{DataFormat, Sink};

pub struct StdoutSink;

#[async_trait]
impl Sink for StdoutSink {
    async fn name(&self) -> &str {
        "stdout"
    }

    async fn write(&self, data: DataFormat, config: &HashMap<String, toml::Value>) -> Result<()> {
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

        Ok(())
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
