use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct JsonExtractTransform;

#[async_trait]
impl Stage for JsonExtractTransform {
    fn name(&self) -> &str {
        "json_extract"
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("JsonExtract transform requires input data"))?;

        let column = config
            .get("column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("JsonExtract requires 'column' configuration"))?;

        let path = config
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("JsonExtract requires 'path' configuration"))?;

        let output_column = config
            .get("output_column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("JsonExtract requires 'output_column' configuration"))?;

        let mut df = data.as_dataframe()?;

        // Get the column containing JSON strings
        let json_column = df.column(column)?.str()?;

        // Parse each JSON string and extract the nested field
        let path_parts: Vec<&str> = path.split('.').collect();
        let extracted_values: Vec<Option<String>> = json_column
            .into_iter()
            .map(|opt_str| {
                opt_str.and_then(|s| {
                    // Parse JSON
                    serde_json::from_str::<Value>(s).ok().and_then(|mut json| {
                        // Navigate the path
                        for part in &path_parts {
                            json = json.get(*part)?.clone();
                        }

                        // Convert to string
                        match json {
                            Value::String(s) => Some(s),
                            Value::Number(n) => Some(n.to_string()),
                            Value::Bool(b) => Some(b.to_string()),
                            Value::Null => None,
                            _ => Some(json.to_string()),
                        }
                    })
                })
            })
            .collect();

        // Create a new Series with the extracted values
        let new_series = Series::new(output_column.into(), extracted_values);

        // Add the new column to the DataFrame
        df.with_column(new_series)?;

        Ok(DataFormat::DataFrame(df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("column") {
            anyhow::bail!("JsonExtract requires 'column' configuration");
        }

        if !config.contains_key("path") {
            anyhow::bail!("JsonExtract requires 'path' configuration");
        }

        if !config.contains_key("output_column") {
            anyhow::bail!("JsonExtract requires 'output_column' configuration");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_json_extract_nested_field() {
        let transform = JsonExtractTransform;

        // Create test data with JSON strings
        let json_data = vec![
            r#"{"meta": {"req": {"headers": {"x-trace-id": "trace-123"}}}}"#,
            r#"{"meta": {"req": {"headers": {"x-trace-id": "trace-456"}}}}"#,
        ];

        let df =
            DataFrame::new(vec![Column::Series(Series::new("Line".into(), json_data))]).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("Line".to_string()),
        );
        config.insert(
            "path".to_string(),
            toml::Value::String("meta.req.headers.x-trace-id".to_string()),
        );
        config.insert(
            "output_column".to_string(),
            toml::Value::String("x_trace_id".to_string()),
        );

        let result = transform.execute(inputs, &config).await.unwrap();
        let result_df = result.as_dataframe().unwrap();

        assert!(result_df.column("x_trace_id").is_ok());
        let x_trace_ids = result_df.column("x_trace_id").unwrap().str().unwrap();
        assert_eq!(x_trace_ids.get(0), Some("trace-123"));
        assert_eq!(x_trace_ids.get(1), Some("trace-456"));
    }

    #[tokio::test]
    async fn test_json_extract_missing_field() {
        let transform = JsonExtractTransform;

        let json_data = vec![
            r#"{"meta": {"req": {}}}"#, // missing headers
            r#"{"other": "data"}"#,     // completely different structure
        ];

        let df =
            DataFrame::new(vec![Column::Series(Series::new("Line".into(), json_data))]).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("Line".to_string()),
        );
        config.insert(
            "path".to_string(),
            toml::Value::String("meta.req.headers.x-trace-id".to_string()),
        );
        config.insert(
            "output_column".to_string(),
            toml::Value::String("x_trace_id".to_string()),
        );

        let result = transform.execute(inputs, &config).await.unwrap();
        let result_df = result.as_dataframe().unwrap();

        let x_trace_ids = result_df.column("x_trace_id").unwrap().str().unwrap();
        assert_eq!(x_trace_ids.get(0), None);
        assert_eq!(x_trace_ids.get(1), None);
    }
}
