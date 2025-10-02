//! HTTP Plugin for Conveyor
//!
//! Provides ABI-stable HTTP source and sink capabilities for REST API integration.

use abi_stable::std_types::{ROption, RString};
use conveyor_plugin_api::*;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Method,
};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// HTTP Source implementation
pub struct HttpSource;

impl SourcePlugin for HttpSource {
    fn name(&self) -> RString {
        "http".into()
    }

    fn read(&self, config: PluginConfig) -> PluginResult<DataFormat> {
        // Use tokio runtime to execute async code
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| PluginError::new(format!("Failed to create runtime: {}", e)))?;

        runtime.block_on(async { self.read_async(&config).await })
    }

    fn validate_config(&self, config: &PluginConfig) -> PluginResult<()> {
        if !config.contains_key(&"url".into()) {
            return Err(PluginError::new("HTTP source requires 'url' configuration"));
        }

        if let Ok(Some(method)) = parse_config_value::<String>(config, "method") {
            let method_upper = method.to_uppercase();
            if !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"]
                .contains(&method_upper.as_str())
            {
                return Err(PluginError::new(format!("Invalid HTTP method: {}", method)));
            }
        }

        if let Ok(Some(format)) = parse_config_value::<String>(config, "format") {
            let valid_formats = ["json", "jsonl"];
            if !valid_formats.contains(&format.as_str()) {
                return Err(PluginError::new(format!(
                    "Invalid HTTP format: {}. Must be one of: {:?}",
                    format, valid_formats
                )));
            }
        }

        Ok(())
    }
}

impl HttpSource {
    async fn read_async(&self, config: &PluginConfig) -> PluginResult<DataFormat> {
        let url: String = parse_config_value(config, "url")
            .map_err(|e| PluginError::new(e))?
            .ok_or_else(|| PluginError::new("Missing 'url' configuration"))?;

        let method: String = parse_config_value(config, "method")
            .map_err(|e| PluginError::new(e))?
            .unwrap_or_else(|| "GET".to_string());

        let timeout_secs: u64 = parse_config_value(config, "timeout")
            .map_err(|e| PluginError::new(e))?
            .unwrap_or(30);

        // Build client with timeout
        let mut client_builder = Client::builder().timeout(Duration::from_secs(timeout_secs));

        // Add headers if provided
        let mut headers = HeaderMap::new();
        if let Ok(Some(headers_str)) = parse_config_value::<String>(config, "headers") {
            // Parse headers as TOML table
            let headers_value: toml::Value = toml::from_str(&headers_str)
                .map_err(|e| PluginError::new(format!("Failed to parse headers: {}", e)))?;

            if let Some(headers_table) = headers_value.as_table() {
                for (key, value) in headers_table {
                    if let Some(val_str) = value.as_str() {
                        let header_name: HeaderName = key
                            .parse()
                            .map_err(|_| PluginError::new(format!("Invalid header name: {}", key)))?;
                        let header_value: HeaderValue = val_str.parse().map_err(|_| {
                            PluginError::new(format!("Invalid header value for {}", key))
                        })?;
                        headers.insert(header_name, header_value);
                    }
                }
            }
        }

        if !headers.is_empty() {
            client_builder = client_builder.default_headers(headers);
        }

        let client = client_builder
            .build()
            .map_err(|e| PluginError::new(format!("Failed to build HTTP client: {}", e)))?;

        // Build request
        let method_enum = method
            .parse::<Method>()
            .map_err(|_| PluginError::new(format!("Invalid HTTP method: {}", method)))?;

        let mut request = client.request(method_enum, &url);

        // Add body for POST/PUT/PATCH requests
        if let Ok(Some(body_str)) = parse_config_value::<String>(config, "body") {
            request = request.body(body_str);
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| PluginError::new(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(PluginError::new(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let response_text = response
            .text()
            .await
            .map_err(|e| PluginError::new(format!("Failed to read response: {}", e)))?;

        // Determine format from config or content-type header
        let format: String = parse_config_value(config, "format")
            .map_err(|e| PluginError::new(e))?
            .unwrap_or_else(|| {
                if content_type.contains("json") {
                    "json".to_string()
                } else {
                    "jsonl".to_string()
                }
            });

        // Parse response based on format
        match format.as_str() {
            "json" => {
                let json_value: Value = serde_json::from_str(&response_text)
                    .map_err(|e| PluginError::new(format!("Failed to parse JSON: {}", e)))?;

                if let Some(array) = json_value.as_array() {
                    let records: Vec<HashMap<String, Value>> = array
                        .iter()
                        .filter_map(|v| {
                            if let Some(obj) = v.as_object() {
                                Some(obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                            } else {
                                None
                            }
                        })
                        .collect();
                    Ok(DataFormat::from_json_records(records))
                } else if let Some(obj) = json_value.as_object() {
                    let record: HashMap<String, Value> =
                        obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    Ok(DataFormat::from_json_records(vec![record]))
                } else {
                    Err(PluginError::new("Unexpected JSON format - expected array or object"))
                }
            }
            "jsonl" => {
                let records: Vec<HashMap<String, Value>> = response_text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| {
                        serde_json::from_str(line).map_err(|e| {
                            PluginError::new(format!("Failed to parse JSONL line: {}", e))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(DataFormat::from_json_records(records))
            }
            _ => Err(PluginError::new(format!(
                "Unknown HTTP format: {}. Use 'json' or 'jsonl'",
                format
            ))),
        }
    }
}

/// HTTP Sink implementation
pub struct HttpSink;

impl SinkPlugin for HttpSink {
    fn name(&self) -> RString {
        "http".into()
    }

    fn write(&self, data: DataFormat, config: PluginConfig) -> PluginResult<()> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| PluginError::new(format!("Failed to create runtime: {}", e)))?;

        runtime.block_on(async { self.write_async(data, &config).await })
    }

    fn validate_config(&self, config: &PluginConfig) -> PluginResult<()> {
        if !config.contains_key(&"url".into()) {
            return Err(PluginError::new("HTTP sink requires 'url' configuration"));
        }

        if let Ok(Some(method)) = parse_config_value::<String>(config, "method") {
            let method_upper = method.to_uppercase();
            if !["POST", "PUT", "PATCH"].contains(&method_upper.as_str()) {
                return Err(PluginError::new(format!(
                    "Invalid HTTP method for sink: {}. Must be POST, PUT, or PATCH",
                    method
                )));
            }
        }

        Ok(())
    }
}

impl HttpSink {
    async fn write_async(&self, data: DataFormat, config: &PluginConfig) -> PluginResult<()> {
        let url: String = parse_config_value(config, "url")
            .map_err(|e| PluginError::new(e))?
            .ok_or_else(|| PluginError::new("Missing 'url' configuration"))?;

        let method: String = parse_config_value(config, "method")
            .map_err(|e| PluginError::new(e))?
            .unwrap_or_else(|| "POST".to_string());

        let timeout_secs: u64 = parse_config_value(config, "timeout")
            .map_err(|e| PluginError::new(e))?
            .unwrap_or(30);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| PluginError::new(format!("Failed to build HTTP client: {}", e)))?;

        let records = data
            .to_json_records()
            .map_err(|e| PluginError::new(format!("Failed to convert data: {}", e)))?;

        let format: String = parse_config_value(config, "format")
            .map_err(|e| PluginError::new(e))?
            .unwrap_or_else(|| "json".to_string());

        let body = match format.as_str() {
            "json" => serde_json::to_string(&records)
                .map_err(|e| PluginError::new(format!("Failed to serialize JSON: {}", e)))?,
            "jsonl" => records
                .iter()
                .map(|r| serde_json::to_string(r))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| PluginError::new(format!("Failed to serialize JSONL: {}", e)))?
                .join("\n"),
            _ => {
                return Err(PluginError::new(format!(
                    "Invalid format: {}. Use 'json' or 'jsonl'",
                    format
                )))
            }
        };

        let method_enum = method
            .parse::<Method>()
            .map_err(|_| PluginError::new(format!("Invalid HTTP method: {}", method)))?;

        let response = client
            .request(method_enum, &url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| PluginError::new(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(PluginError::new(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        Ok(())
    }
}

// Export plugin using the macro
conveyor_plugin_api::export_plugin! {
    name: "http",
    version: env!("CARGO_PKG_VERSION"),
    description: "HTTP plugin for REST API integration",
    source: HttpSource,
    sink: HttpSink,
}
