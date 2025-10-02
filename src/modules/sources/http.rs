use crate::core::traits::{DataFormat, DataSource, RecordBatch};
use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, Client, Method};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub struct HttpSource;

#[async_trait]
impl DataSource for HttpSource {
    async fn name(&self) -> &str {
        "http"
    }

    async fn read(&self, config: &HashMap<String, toml::Value>) -> Result<DataFormat> {
        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("HTTP source requires 'url' configuration"))?;

        let method = config
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();

        let timeout_secs = config
            .get("timeout")
            .and_then(|v| v.as_integer())
            .unwrap_or(30) as u64;

        // Build client with timeout
        let mut client_builder = Client::builder().timeout(Duration::from_secs(timeout_secs));

        // Add headers if provided
        let mut headers = HeaderMap::new();
        if let Some(header_config) = config.get("headers") {
            if let Some(header_table) = header_config.as_table() {
                for (key, value) in header_table {
                    if let Some(val_str) = value.as_str() {
                        let header_name: HeaderName = key
                            .parse()
                            .map_err(|_| anyhow::anyhow!("Invalid header name: {}", key))?;
                        let header_value: HeaderValue = val_str
                            .parse()
                            .map_err(|_| anyhow::anyhow!("Invalid header value for {}", key))?;
                        headers.insert(header_name, header_value);
                    }
                }
            }
        }

        if !headers.is_empty() {
            client_builder = client_builder.default_headers(headers);
        }

        let client = client_builder.build()?;

        // Build request
        let method_enum = method
            .parse::<Method>()
            .map_err(|_| anyhow::anyhow!("Invalid HTTP method: {}", method))?;

        let mut request = client.request(method_enum, url);

        // Add body for POST/PUT/PATCH requests
        if let Some(body_config) = config.get("body") {
            if let Some(body_str) = body_config.as_str() {
                request = request.body(body_str.to_string());
            } else if let Some(body_table) = body_config.as_table() {
                let json_body = toml::Value::Table(body_table.clone());
                let json_str = serde_json::to_string(&json_body)?;
                request = request
                    .header("Content-Type", "application/json")
                    .body(json_str);
            }
        }

        // Send request
        tracing::info!("Sending {} request to {}", method, url);
        let response = request.send().await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "HTTP request failed with status: {}",
                response.status()
            );
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let response_text = response.text().await?;

        // Determine format from config or content-type header
        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                if content_type.contains("json") {
                    "json"
                } else if content_type.contains("csv") {
                    "csv"
                } else {
                    "raw"
                }
            });

        // Parse response based on format (similar to stdin source)
        match format {
            "json" => {
                let json_value: Value = serde_json::from_str(&response_text)?;

                if let Some(array) = json_value.as_array() {
                    let records: RecordBatch = array
                        .iter()
                        .filter_map(|v| {
                            if let Some(obj) = v.as_object() {
                                Some(
                                    obj.iter()
                                        .map(|(k, v)| (k.clone(), v.clone()))
                                        .collect(),
                                )
                            } else {
                                None
                            }
                        })
                        .collect();
                    tracing::info!("Received {} JSON records", records.len());
                    Ok(DataFormat::RecordBatch(records))
                } else if let Some(obj) = json_value.as_object() {
                    let record: HashMap<String, Value> =
                        obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    tracing::info!("Received single JSON object");
                    Ok(DataFormat::RecordBatch(vec![record]))
                } else {
                    anyhow::bail!("Unexpected JSON format - expected array or object")
                }
            }
            "jsonl" => {
                let records: RecordBatch = response_text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| serde_json::from_str(line))
                    .collect::<Result<Vec<_>, _>>()?;
                tracing::info!("Received {} JSONL records", records.len());
                Ok(DataFormat::RecordBatch(records))
            }
            "csv" => {
                let cursor = std::io::Cursor::new(response_text.as_bytes());

                let has_headers = config
                    .get("headers")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let df = CsvReadOptions::default()
                    .with_has_header(has_headers)
                    .into_reader_with_file_handle(cursor)
                    .finish()?;

                tracing::info!("Received CSV with {} rows", df.height());
                Ok(DataFormat::DataFrame(df))
            }
            "raw" => {
                tracing::info!("Received {} bytes of raw data", response_text.len());
                Ok(DataFormat::Raw(response_text.into_bytes()))
            }
            _ => anyhow::bail!(
                "Unknown HTTP format: {}. Use 'json', 'jsonl', 'csv', or 'raw'",
                format
            ),
        }
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("url") {
            anyhow::bail!("HTTP source requires 'url' configuration");
        }

        if let Some(method) = config.get("method").and_then(|v| v.as_str()) {
            let method_upper = method.to_uppercase();
            if !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"]
                .contains(&method_upper.as_str())
            {
                anyhow::bail!("Invalid HTTP method: {}", method);
            }
        }

        if let Some(format) = config.get("format").and_then(|v| v.as_str()) {
            let valid_formats = ["json", "jsonl", "csv", "raw"];
            if !valid_formats.contains(&format) {
                anyhow::bail!(
                    "Invalid HTTP format: {}. Must be one of: {:?}",
                    format,
                    valid_formats
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_config_missing_url() {
        let source = HttpSource;
        let config = HashMap::new();
        assert!(source.validate_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_validate_config_valid() {
        let source = HttpSource;
        let mut config = HashMap::new();
        config.insert(
            "url".to_string(),
            toml::Value::String("https://api.example.com".to_string()),
        );
        config.insert("method".to_string(), toml::Value::String("GET".to_string()));
        assert!(source.validate_config(&config).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_invalid_method() {
        let source = HttpSource;
        let mut config = HashMap::new();
        config.insert(
            "url".to_string(),
            toml::Value::String("https://api.example.com".to_string()),
        );
        config.insert(
            "method".to_string(),
            toml::Value::String("INVALID".to_string()),
        );
        assert!(source.validate_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_validate_config_invalid_format() {
        let source = HttpSource;
        let mut config = HashMap::new();
        config.insert(
            "url".to_string(),
            toml::Value::String("https://api.example.com".to_string()),
        );
        config.insert(
            "format".to_string(),
            toml::Value::String("xml".to_string()),
        );
        assert!(source.validate_config(&config).await.is_err());
    }
}
