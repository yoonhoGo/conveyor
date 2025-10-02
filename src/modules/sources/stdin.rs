use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;
use std::io::{self, Read};

use crate::core::traits::{DataFormat, DataSource, RecordBatch};

pub struct StdinSource;

#[async_trait]
impl DataSource for StdinSource {
    async fn name(&self) -> &str {
        "stdin"
    }

    async fn read(&self, config: &HashMap<String, toml::Value>) -> Result<DataFormat> {
        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("json");

        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

        match format {
            "json" => {
                let records: RecordBatch = serde_json::from_str(&buffer)?;
                Ok(DataFormat::RecordBatch(records))
            }
            "jsonl" => {
                let records: RecordBatch = buffer
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| serde_json::from_str(line))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(DataFormat::RecordBatch(records))
            }
            "csv" => {
                let cursor = std::io::Cursor::new(buffer.as_bytes());

                let has_headers = config
                    .get("headers")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let _delimiter = config
                    .get("delimiter")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.chars().next())
                    .unwrap_or(',') as u8;

                let reader_builder = CsvReadOptions::default().with_has_header(has_headers);

                let df = reader_builder
                    .into_reader_with_file_handle(cursor)
                    .finish()?;

                Ok(DataFormat::DataFrame(df))
            }
            "raw" => Ok(DataFormat::Raw(buffer.into_bytes())),
            _ => anyhow::bail!(
                "Unknown stdin format: {}. Use 'json', 'jsonl', 'csv', or 'raw'",
                format
            ),
        }
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if let Some(format) = config.get("format") {
            if let Some(fmt_str) = format.as_str() {
                let valid_formats = ["json", "jsonl", "csv", "raw"];
                if !valid_formats.contains(&fmt_str) {
                    anyhow::bail!(
                        "Invalid stdin format: {}. Must be one of: {:?}",
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
