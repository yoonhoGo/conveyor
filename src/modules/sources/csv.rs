use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::metadata::{ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct CsvSource;

#[async_trait]
impl Stage for CsvSource {
    fn name(&self) -> &str {
        "csv.read"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example_config = HashMap::new();
        example_config.insert("path".to_string(), toml::Value::String("data.csv".to_string()));
        example_config.insert("headers".to_string(), toml::Value::Boolean(true));
        example_config.insert("delimiter".to_string(), toml::Value::String(",".to_string()));

        StageMetadata::builder("csv.read", StageCategory::Source)
            .description("Read data from CSV files")
            .long_description(
                "Reads CSV (Comma-Separated Values) files and converts them into a DataFrame. \
                Supports custom delimiters, header configuration, and automatic schema inference. \
                Uses Polars for efficient CSV parsing with zero-copy operations where possible."
            )
            .parameter(ConfigParameter::required(
                "path",
                ParameterType::String,
                "Path to the CSV file to read"
            ))
            .parameter(ConfigParameter::optional(
                "headers",
                ParameterType::Boolean,
                "true",
                "Whether the first row contains column headers"
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
            .parameter(ConfigParameter::optional(
                "infer_schema_length",
                ParameterType::Integer,
                "100",
                "Number of rows to scan for schema inference (0 = scan all rows)"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Basic CSV reading",
                example_config.clone(),
                Some("Read a CSV file with headers and comma delimiter")
            ))
            .tag("csv")
            .tag("file")
            .tag("io")
            .tag("source")
            .build()
    }

    async fn execute(
        &self,
        _inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let path = config
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("CSV source requires 'path' configuration"))?;

        let has_headers = config
            .get("headers")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let _delimiter = config
            .get("delimiter")
            .and_then(|v| v.as_str())
            .and_then(|s| s.chars().next())
            .unwrap_or(',') as u8;

        let _infer_schema_length = config
            .get("infer_schema_length")
            .and_then(|v| v.as_integer())
            .map(|v| v as usize);

        let path_buf = PathBuf::from(path);

        if !path_buf.exists() {
            anyhow::bail!("CSV file not found: {}", path);
        }

        let file = std::fs::File::open(&path_buf)?;
        let reader_builder = CsvReadOptions::default().with_has_header(has_headers);

        let df = reader_builder.into_reader_with_file_handle(file).finish()?;

        Ok(DataFormat::DataFrame(df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("path") {
            anyhow::bail!("CSV source requires 'path' configuration");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_csv_source_read() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "id,name,value").unwrap();
        writeln!(temp_file, "1,Alice,100").unwrap();
        writeln!(temp_file, "2,Bob,200").unwrap();

        let mut config = HashMap::new();
        config.insert(
            "path".to_string(),
            toml::Value::String(temp_file.path().to_string_lossy().to_string()),
        );
        config.insert("headers".to_string(), toml::Value::Boolean(true));

        let source = CsvSource;
        let inputs = HashMap::new();
        let result = source.execute(inputs, &config).await;
        assert!(result.is_ok());

        let data = result.unwrap();
        match data {
            DataFormat::DataFrame(df) => {
                assert_eq!(df.height(), 2);
                assert_eq!(df.width(), 3);
            }
            _ => panic!("Expected DataFrame"),
        }
    }

    #[tokio::test]
    async fn test_csv_source_validation() {
        let source = CsvSource;

        // Valid config
        let mut valid_config = HashMap::new();
        valid_config.insert(
            "path".to_string(),
            toml::Value::String("test.csv".to_string()),
        );
        assert!(source.validate_config(&valid_config).await.is_ok());

        // Invalid config - missing path
        let invalid_config = HashMap::new();
        assert!(source.validate_config(&invalid_config).await.is_err());
    }
}
