use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;
use std::io::{self, Read};

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::traits::{DataFormat, RecordBatch};

pub struct StdinSource;

#[async_trait]
impl Stage for StdinSource {
    fn name(&self) -> &str {
        "stdin.read"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert(
            "format".to_string(),
            toml::Value::String("json".to_string()),
        );

        let mut example2 = HashMap::new();
        example2.insert("format".to_string(), toml::Value::String("csv".to_string()));
        example2.insert("headers".to_string(), toml::Value::Boolean(true));

        StageMetadata::builder("stdin.read", StageCategory::Source)
            .description("Read data from standard input")
            .long_description(
                "Reads data from stdin, enabling pipeline composition with Unix tools. \
                Supports JSON, JSON Lines, CSV, and raw binary formats. \
                Ideal for integrating Conveyor with shell scripts and command-line workflows.",
            )
            .parameter(
                ConfigParameter::optional(
                    "format",
                    ParameterType::String,
                    "json",
                    "Input data format",
                )
                .with_validation(ParameterValidation::allowed_values([
                    "json", "jsonl", "csv", "raw",
                ])),
            )
            .parameter(ConfigParameter::optional(
                "headers",
                ParameterType::Boolean,
                "true",
                "Whether CSV input has headers (only applies to 'csv' format)",
            ))
            .parameter(ConfigParameter::optional(
                "delimiter",
                ParameterType::String,
                ",",
                "Field delimiter for CSV (only applies to 'csv' format)",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Read JSON from stdin",
                example1,
                Some("cat data.json | conveyor run pipeline.toml"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Read CSV from stdin",
                example2,
                Some("cat data.csv | conveyor run pipeline.toml"),
            ))
            .tag("stdin")
            .tag("input")
            .tag("pipe")
            .tag("source")
            .build()
    }

    async fn execute(
        &self,
        _inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
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
                    .map(serde_json::from_str)
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
