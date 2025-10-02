use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::traits::{DataFormat, Sink};

pub struct CsvSink;

#[async_trait]
impl Sink for CsvSink {
    async fn name(&self) -> &str {
        "csv"
    }

    async fn write(&self, data: DataFormat, config: &HashMap<String, toml::Value>) -> Result<()> {
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

        Ok(())
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
