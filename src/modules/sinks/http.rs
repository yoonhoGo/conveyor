use crate::core::traits::{DataFormat, Sink};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, Client, Method};
use std::collections::HashMap;
use std::time::Duration;

pub struct HttpSink;

#[async_trait]
impl Sink for HttpSink {
    async fn name(&self) -> &str {
        "http"
    }

    async fn write(
        &self,
        data: DataFormat,
        config: &HashMap<String, toml::Value>,
    ) -> Result<()> {
        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("HTTP sink requires 'url' configuration"))?;

        let method = config
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("POST")
            .to_uppercase();

        let timeout_secs = config
            .get("timeout")
            .and_then(|v| v.as_integer())
            .unwrap_or(30) as u64;

        let format = config
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("json");

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

        // Convert data to appropriate format and build request body
        let body = match format {
            "json" => {
                let records = data.as_record_batch()?;
                let json_str = serde_json::to_string(&records)?;
                request = request.header("Content-Type", "application/json");
                json_str
            }
            "jsonl" => {
                let records = data.as_record_batch()?;
                let jsonl_str = records
                    .iter()
                    .map(|record| serde_json::to_string(record))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\n");
                request = request.header("Content-Type", "application/x-ndjson");
                jsonl_str
            }
            _ => {
                anyhow::bail!(
                    "Unknown HTTP sink format: {}. Use 'json' or 'jsonl'",
                    format
                )
            }
        };

        request = request.body(body);

        // Send request
        let row_count = match &data {
            DataFormat::DataFrame(df) => df.height(),
            DataFormat::RecordBatch(records) => records.len(),
            _ => 0,
        };

        tracing::info!("Sending {} request to {} with {} rows", method, url, row_count);
        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "HTTP request failed with status {}: {}",
                status,
                error_body
            );
        }

        tracing::info!("Successfully sent {} rows to {}", row_count, url);

        Ok(())
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("url") {
            anyhow::bail!("HTTP sink requires 'url' configuration");
        }

        if let Some(method) = config.get("method").and_then(|v| v.as_str()) {
            let method_upper = method.to_uppercase();
            if !["POST", "PUT", "PATCH"].contains(&method_upper.as_str()) {
                anyhow::bail!(
                    "Invalid HTTP sink method: {}. Use 'POST', 'PUT', or 'PATCH'",
                    method
                );
            }
        }

        if let Some(format) = config.get("format").and_then(|v| v.as_str()) {
            let valid_formats = ["json", "jsonl"];
            if !valid_formats.contains(&format) {
                anyhow::bail!(
                    "Invalid HTTP sink format: {}. Must be one of: {:?}",
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
        let sink = HttpSink;
        let config = HashMap::new();
        assert!(sink.validate_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_validate_config_valid() {
        let sink = HttpSink;
        let mut config = HashMap::new();
        config.insert(
            "url".to_string(),
            toml::Value::String("https://api.example.com".to_string()),
        );
        config.insert(
            "method".to_string(),
            toml::Value::String("POST".to_string()),
        );
        assert!(sink.validate_config(&config).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_invalid_method() {
        let sink = HttpSink;
        let mut config = HashMap::new();
        config.insert(
            "url".to_string(),
            toml::Value::String("https://api.example.com".to_string()),
        );
        config.insert("method".to_string(), toml::Value::String("GET".to_string()));
        assert!(sink.validate_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn test_validate_config_invalid_format() {
        let sink = HttpSink;
        let mut config = HashMap::new();
        config.insert(
            "url".to_string(),
            toml::Value::String("https://api.example.com".to_string()),
        );
        config.insert("format".to_string(), toml::Value::String("xml".to_string()));
        assert!(sink.validate_config(&config).await.is_err());
    }
}
