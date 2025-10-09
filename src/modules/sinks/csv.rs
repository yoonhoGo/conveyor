use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct CsvSink;

#[async_trait]
impl Stage for CsvSink {
    fn name(&self) -> &str {
        "csv.write"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example_config = HashMap::new();
        example_config.insert(
            "path".to_string(),
            toml::Value::String("output.csv".to_string()),
        );
        example_config.insert("headers".to_string(), toml::Value::Boolean(true));
        example_config.insert(
            "delimiter".to_string(),
            toml::Value::String(",".to_string()),
        );

        StageMetadata::builder("csv.write", StageCategory::Sink)
            .description("Write data to CSV files")
            .long_description(
                "Writes DataFrame to CSV (Comma-Separated Values) files with customizable options. \
                Supports custom delimiters, header configuration, and automatic directory creation. \
                Uses Polars' efficient CSV writer for high-performance output."
            )
            .parameter(ConfigParameter::required(
                "path",
                ParameterType::String,
                "Path to the output CSV file"
            ))
            .parameter(ConfigParameter::optional(
                "headers",
                ParameterType::Boolean,
                "true",
                "Whether to include column headers in the first row"
            ))
            .parameter(ConfigParameter::optional(
                "delimiter",
                ParameterType::String,
                ",",
                "Field delimiter character (must be a single character)"
            ).with_validation(ParameterValidation {
                min: None,
                max: None,
                allowed_values: None,
                pattern: None,
                min_length: Some(1),
                max_length: Some(1),
            }))
            .example(crate::core::metadata::ConfigExample::new(
                "Standard CSV output",
                example_config,
                Some("Write data to CSV with headers and comma delimiter")
            ))
            .tag("csv")
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
        // Get input data
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("CSV sink requires input data"))?;
        let path = config
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("CSV sink requires 'path' configuration"))?;

        let has_headers = config
            .get("headers")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let delimiter = config
            .get("delimiter")
            .and_then(|v| v.as_str())
            .and_then(|s| s.chars().next())
            .unwrap_or(',') as u8;

        let path_buf = PathBuf::from(path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = path_buf.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let df = data.as_dataframe()?;

        // Write DataFrame to CSV
        let mut file = std::fs::File::create(&path_buf)?;
        CsvWriter::new(&mut file)
            .include_header(has_headers)
            .with_separator(delimiter)
            .finish(&mut df.clone())?;

        tracing::info!("Written {} rows to CSV file: {}", df.height(), path);

        // Sinks return empty RecordBatch
        Ok(DataFormat::RecordBatch(vec![]))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("path") {
            anyhow::bail!("CSV sink requires 'path' configuration");
        }

        if let Some(delimiter) = config.get("delimiter") {
            if let Some(delim_str) = delimiter.as_str() {
                if delim_str.len() != 1 {
                    anyhow::bail!("Delimiter must be a single character");
                }
            } else {
                anyhow::bail!("Delimiter must be a string");
            }
        }

        Ok(())
    }
}
