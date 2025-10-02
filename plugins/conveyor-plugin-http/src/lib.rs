//! HTTP Plugin for Conveyor
//!
//! Provides FFI-safe HTTP source and sink capabilities for REST API integration.

use conveyor_plugin_api::{
    data::FfiDataFormat,
    rstr,
    traits::{FfiDataSource, FfiSink},
    PluginDeclaration, RBoxError, RHashMap, RResult, RStr, RString, ROk, RErr,
};
use reqwest::{Client, Method};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

// ============================================================================
// HTTP Source Implementation
// ============================================================================

/// HTTP Source - fetches data from REST APIs
struct HttpSource;

impl FfiDataSource for HttpSource {
    fn name(&self) -> RStr<'_> {
        "http_source".into()
    }

    fn read(&self, config: RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to create runtime: {}", e))),
        };

        runtime.block_on(async { self.read_async(&config).await })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        if !config.contains_key(&RString::from("url")) {
            return RErr(RBoxError::from_fmt(&format_args!("HTTP source requires 'url' configuration")));
        }

        if let Some(method) = config.get(&RString::from("method")) {
            let method_upper = method.as_str().to_uppercase();
            if !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"].contains(&method_upper.as_str()) {
                return RErr(RBoxError::from_fmt(&format_args!("Invalid HTTP method: {}", method)));
            }
        }

        ROk(())
    }
}

impl HttpSource {
    async fn read_async(&self, config: &RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
        // Get URL
        let url = match config.get(&RString::from("url")) {
            Some(u) => u.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'url' configuration"))),
        };

        // Get method (default: GET)
        let method = config
            .get(&RString::from("method"))
            .map(|m| m.as_str())
            .unwrap_or("GET");

        // Get timeout (default: 30 seconds)
        let timeout_secs: u64 = config
            .get(&RString::from("timeout"))
            .and_then(|t| t.as_str().parse().ok())
            .unwrap_or(30);

        // Build client
        let client = match Client::builder().timeout(Duration::from_secs(timeout_secs)).build() {
            Ok(c) => c,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to build HTTP client: {}", e))),
        };

        // Parse method
        let method_enum = match method.parse::<Method>() {
            Ok(m) => m,
            Err(_) => return RErr(RBoxError::from_fmt(&format_args!("Invalid HTTP method: {}", method))),
        };

        // Build request
        let mut request = client.request(method_enum, url);

        // Add body for POST/PUT/PATCH
        if let Some(body) = config.get(&RString::from("body")) {
            request = request.body(body.as_str().to_string());
        }

        // Send request
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("HTTP request failed: {}", e))),
        };

        if !response.status().is_success() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Get response text
        let response_text = match response.text().await {
            Ok(t) => t,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to read response: {}", e))),
        };

        // Determine format (default: json)
        let format = config
            .get(&RString::from("format"))
            .map(|f| f.as_str())
            .unwrap_or("json");

        // Parse response based on format
        match format {
            "json" => {
                let json_value: Value = match serde_json::from_str(&response_text) {
                    Ok(v) => v,
                    Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to parse JSON: {}", e))),
                };

                let records = if let Some(array) = json_value.as_array() {
                    array
                        .iter()
                        .filter_map(|v| {
                            v.as_object().map(|obj| {
                                obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                            })
                        })
                        .collect::<Vec<HashMap<String, Value>>>()
                } else if let Some(obj) = json_value.as_object() {
                    vec![obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()]
                } else {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Unexpected JSON format - expected array or object"
                    )));
                };

                FfiDataFormat::from_json_records(&records)
            }
            "jsonl" => {
                let records: Result<Vec<HashMap<String, Value>>, _> = response_text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| serde_json::from_str(line))
                    .collect();

                match records {
                    Ok(r) => FfiDataFormat::from_json_records(&r),
                    Err(e) => RErr(RBoxError::from_fmt(&format_args!("Failed to parse JSONL: {}", e))),
                }
            }
            _ => RErr(RBoxError::from_fmt(&format_args!(
                "Unknown format: {}. Use 'json' or 'jsonl'",
                format
            ))),
        }
    }
}

// ============================================================================
// HTTP Sink Implementation
// ============================================================================

/// HTTP Sink - sends data to REST APIs
struct HttpSink;

impl FfiSink for HttpSink {
    fn name(&self) -> RStr<'_> {
        "http_sink".into()
    }

    fn write(&self, data: FfiDataFormat, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to create runtime: {}", e))),
        };

        runtime.block_on(async { self.write_async(data, &config).await })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        if !config.contains_key(&RString::from("url")) {
            return RErr(RBoxError::from_fmt(&format_args!("HTTP sink requires 'url' configuration")));
        }

        if let Some(method) = config.get(&RString::from("method")) {
            let method_upper = method.as_str().to_uppercase();
            if !["POST", "PUT", "PATCH"].contains(&method_upper.as_str()) {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid HTTP method for sink: {}. Must be POST, PUT, or PATCH",
                    method
                )));
            }
        }

        ROk(())
    }
}

impl HttpSink {
    async fn write_async(&self, data: FfiDataFormat, config: &RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Get URL
        let url = match config.get(&RString::from("url")) {
            Some(u) => u.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'url' configuration"))),
        };

        // Get method (default: POST)
        let method = config
            .get(&RString::from("method"))
            .map(|m| m.as_str())
            .unwrap_or("POST");

        // Get timeout (default: 30 seconds)
        let timeout_secs: u64 = config
            .get(&RString::from("timeout"))
            .and_then(|t| t.as_str().parse().ok())
            .unwrap_or(30);

        // Build client
        let client = match Client::builder().timeout(Duration::from_secs(timeout_secs)).build() {
            Ok(c) => c,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to build HTTP client: {}", e))),
        };

        // Convert data to JSON records
        let records = match data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        // Get format (default: json)
        let format = config
            .get(&RString::from("format"))
            .map(|f| f.as_str())
            .unwrap_or("json");

        // Serialize body based on format
        let body = match format {
            "json" => match serde_json::to_string(&records) {
                Ok(s) => s,
                Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to serialize JSON: {}", e))),
            },
            "jsonl" => {
                let lines: Result<Vec<String>, _> = records
                    .iter()
                    .map(|r| serde_json::to_string(r))
                    .collect();

                match lines {
                    Ok(l) => l.join("\n"),
                    Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to serialize JSONL: {}", e))),
                }
            }
            _ => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid format: {}. Use 'json' or 'jsonl'",
                    format
                )))
            }
        };

        // Parse method
        let method_enum = match method.parse::<Method>() {
            Ok(m) => m,
            Err(_) => return RErr(RBoxError::from_fmt(&format_args!("Invalid HTTP method: {}", method))),
        };

        // Send request
        let response = match client
            .request(method_enum, url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("HTTP request failed: {}", e))),
        };

        if !response.status().is_success() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        ROk(())
    }
}

// ============================================================================
// Plugin Registration
// ============================================================================

extern "C" fn register() -> RResult<(), RBoxError> {
    // Future: Register modules with host registry
    ROk(())
}

/// Plugin declaration - exported symbol
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: conveyor_plugin_api::PLUGIN_API_VERSION,
    name: rstr!("http"),
    version: rstr!("0.1.0"),
    description: rstr!("HTTP plugin for REST API integration"),
    register,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_source() {
        let source = HttpSource;
        assert_eq!(source.name(), "http_source");
    }

    #[test]
    fn test_http_sink() {
        let sink = HttpSink;
        assert_eq!(sink.name(), "http_sink");
    }

    #[test]
    fn test_plugin_declaration() {
        assert_eq!(_plugin_declaration.name, "http");
        assert_eq!(_plugin_declaration.version, "0.1.0");
        assert!(_plugin_declaration.is_compatible());
    }

    #[test]
    fn test_source_validation() {
        let source = HttpSource;
        let mut config = RHashMap::new();

        // Missing URL should fail
        assert!(source.validate_config(config.clone()).is_err());

        // With URL should succeed
        config.insert(RString::from("url"), RString::from("https://api.example.com"));
        assert!(source.validate_config(config.clone()).is_ok());

        // Invalid method should fail
        config.insert(RString::from("method"), RString::from("INVALID"));
        assert!(source.validate_config(config).is_err());
    }

    #[test]
    fn test_sink_validation() {
        let sink = HttpSink;
        let mut config = RHashMap::new();

        // Missing URL should fail
        assert!(sink.validate_config(config.clone()).is_err());

        // With URL should succeed
        config.insert(RString::from("url"), RString::from("https://api.example.com"));
        assert!(sink.validate_config(config.clone()).is_ok());

        // GET method should fail for sink
        config.insert(RString::from("method"), RString::from("GET"));
        assert!(sink.validate_config(config).is_err());
    }
}
