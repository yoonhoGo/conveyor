//! HTTP Plugin for Conveyor - Version 2 (Unified Stage API)
//!
//! Provides HTTP source and sink functionality as a unified stage.
//! Supports multiple HTTP methods and data formats.

use conveyor_plugin_api::sabi_trait::prelude::*;
use conveyor_plugin_api::traits::{FfiExecutionContext, FfiStage, FfiStage_TO};
use conveyor_plugin_api::{
    rstr, FfiDataFormat, PluginCapability, PluginDeclaration, RBox, RBoxError, RErr, RHashMap, ROk,
    RResult, RString, RVec, StageType, PLUGIN_API_VERSION,
};
use reqwest::{Client, Method};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// HTTP Stage - unified source and sink
pub struct HttpStage {
    name: String,
    stage_type: StageType,
}

impl HttpStage {
    fn new(name: String, stage_type: StageType) -> Self {
        Self { name, stage_type }
    }

    /// Execute as HTTP source (fetch data from URL)
    async fn execute_source_async(
        &self,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        // Get configuration
        let url = match config.get("url") {
            Some(u) => u,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'url' configuration"
                )))
            }
        };

        let method = config.get("method").map(|s| s.as_str()).unwrap_or("GET");
        let format = config.get("format").map(|s| s.as_str()).unwrap_or("json");
        let timeout_secs: u64 = config
            .get("timeout_seconds")
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        // Build HTTP client
        let client = match Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create HTTP client: {}",
                    e
                )))
            }
        };

        // Parse method
        let method_enum = match method.parse::<Method>() {
            Ok(m) => m,
            Err(_) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid HTTP method: {}",
                    method
                )))
            }
        };

        // Build request
        let mut request = client.request(method_enum, url);

        // Add custom headers
        for (key, value) in config.iter() {
            if key.starts_with("header.") {
                let header_name = key.strip_prefix("header.").unwrap();
                request = request.header(header_name, value);
            }
        }

        // Add body if provided
        if let Some(body) = config.get("body") {
            request = request.body(body.clone());
        }

        // Execute request
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "HTTP request failed: {}",
                    e
                )))
            }
        };

        if !response.status().is_success() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Parse response based on format
        match format {
            "json" => {
                let response_text = match response.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Failed to read response: {}",
                            e
                        )))
                    }
                };

                let json: Value = match serde_json::from_str(&response_text) {
                    Ok(v) => v,
                    Err(e) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Failed to parse JSON response: {}",
                            e
                        )))
                    }
                };

                // Convert to records
                let records = if let Some(array) = json.as_array() {
                    array
                        .iter()
                        .filter_map(|v| {
                            if let Some(obj) = v.as_object() {
                                let mut map = HashMap::new();
                                for (k, v) in obj {
                                    map.insert(k.clone(), v.clone());
                                }
                                Some(map)
                            } else {
                                None
                            }
                        })
                        .collect()
                } else if let Some(obj) = json.as_object() {
                    let mut map = HashMap::new();
                    for (k, v) in obj {
                        map.insert(k.clone(), v.clone());
                    }
                    vec![map]
                } else {
                    return RErr(RBoxError::from_fmt(&format_args!("Unexpected JSON format")));
                };

                FfiDataFormat::from_json_records(&records)
            }
            "jsonl" => {
                let text = match response.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Failed to read response text: {}",
                            e
                        )))
                    }
                };

                let records: Result<Vec<HashMap<String, Value>>, _> = text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(serde_json::from_str)
                    .collect();

                match records {
                    Ok(r) => FfiDataFormat::from_json_records(&r),
                    Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                        "Failed to parse JSONL: {}",
                        e
                    ))),
                }
            }
            "raw" => {
                let bytes = match response.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Failed to read response bytes: {}",
                            e
                        )))
                    }
                };

                ROk(FfiDataFormat::from_raw(bytes))
            }
            _ => RErr(RBoxError::from_fmt(&format_args!(
                "Unsupported format: {}",
                format
            ))),
        }
    }

    /// Execute as HTTP sink (send data to URL)
    async fn execute_sink_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        // Get configuration
        let url = match config.get("url") {
            Some(u) => u,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'url' configuration"
                )))
            }
        };

        let method = config.get("method").map(|s| s.as_str()).unwrap_or("POST");
        let timeout_secs: u64 = config
            .get("timeout_seconds")
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        // Build HTTP client
        let client = match Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create HTTP client: {}",
                    e
                )))
            }
        };

        // Convert data to JSON records
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        // Get format (default: json)
        let format = config.get("format").map(|s| s.as_str()).unwrap_or("json");

        // Serialize body based on format
        let body = match format {
            "json" => match serde_json::to_string(&records) {
                Ok(s) => s,
                Err(e) => {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Failed to serialize JSON: {}",
                        e
                    )))
                }
            },
            "jsonl" => {
                let lines: Result<Vec<String>, _> =
                    records.iter().map(serde_json::to_string).collect();

                match lines {
                    Ok(l) => l.join("\n"),
                    Err(e) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Failed to serialize JSONL: {}",
                            e
                        )))
                    }
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
            Err(_) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid HTTP method: {}",
                    method
                )))
            }
        };

        // Build request
        let mut request = client
            .request(method_enum, url)
            .header("Content-Type", "application/json")
            .body(body);

        // Add custom headers
        for (key, value) in config.iter() {
            if key.starts_with("header.") {
                let header_name = key.strip_prefix("header.").unwrap();
                request = request.header(header_name, value);
            }
        }

        // Send request
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "HTTP request failed: {}",
                    e
                )))
            }
        };

        if !response.status().is_success() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Return the original data (sinks pass through data)
        ROk(input_data.clone())
    }
}

impl FfiStage for HttpStage {
    fn name(&self) -> conveyor_plugin_api::RStr<'_> {
        self.name.as_str().into()
    }

    fn stage_type(&self) -> StageType {
        self.stage_type
    }

    fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
        // Convert config to HashMap
        let config: HashMap<String, String> = context
            .config
            .into_iter()
            .map(|tuple| (tuple.0.to_string(), tuple.1.to_string()))
            .collect();

        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create runtime: {}",
                    e
                )))
            }
        };

        runtime.block_on(async {
            match self.stage_type {
                StageType::Source => {
                    // Source mode: fetch from HTTP
                    self.execute_source_async(&config).await
                }
                StageType::Sink => {
                    // Sink mode: send to HTTP
                    // Get input data (should have exactly one input)
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "HTTP sink requires input data"
                            )))
                        }
                    };

                    self.execute_sink_async(&input_data, &config).await
                }
                StageType::Transform => RErr(RBoxError::from_fmt(&format_args!(
                    "HTTP transform not supported in this plugin"
                ))),
            }
        })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Check required fields
        if !config.contains_key("url") {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Missing required 'url' configuration"
            )));
        }

        // Validate method if provided
        if let Some(method) = config.get("method") {
            let method_str = method.as_str().to_uppercase();
            match self.stage_type {
                StageType::Source => {
                    if !["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"]
                        .contains(&method_str.as_str())
                    {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Invalid HTTP method: {}",
                            method_str
                        )));
                    }
                }
                StageType::Sink => {
                    if !["POST", "PUT", "PATCH"].contains(&method_str.as_str()) {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Invalid HTTP method for sink: {}",
                            method_str
                        )));
                    }
                }
                _ => {}
            }
        }

        // Validate format if provided
        if let Some(format) = config.get("format") {
            let format_str = format.as_str();
            if !["json", "jsonl", "raw"].contains(&format_str) {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid format: {}",
                    format_str
                )));
            }
        }

        ROk(())
    }
}

// Factory functions
#[no_mangle]
pub extern "C" fn create_http_source() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        HttpStage::new("http".to_string(), StageType::Source),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_http_sink() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        HttpStage::new("http".to_string(), StageType::Sink),
        TD_Opaque,
    )
}

// Plugin capabilities
extern "C" fn get_capabilities() -> RVec<PluginCapability> {
    vec![
        PluginCapability::simple(
            "http",
            StageType::Source,
            "HTTP source - fetch data from REST APIs",
            "create_http_source",
        ),
        PluginCapability::simple(
            "http",
            StageType::Sink,
            "HTTP sink - send data to REST APIs",
            "create_http_sink",
        ),
    ]
    .into()
}

// Plugin declaration
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("http"),
    version: rstr!("1.0.0"),
    description: rstr!("HTTP source and sink plugin for REST API integration"),
    get_capabilities,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_source() {
        let stage = HttpStage::new("http".to_string(), StageType::Source);
        assert_eq!(stage.name(), "http");
        assert_eq!(stage.stage_type(), StageType::Source);
    }

    #[test]
    fn test_http_sink() {
        let stage = HttpStage::new("http".to_string(), StageType::Sink);
        assert_eq!(stage.name(), "http");
        assert_eq!(stage.stage_type(), StageType::Sink);
    }

    #[test]
    fn test_plugin_declaration() {
        assert_eq!(_plugin_declaration.name, "http");
        assert_eq!(_plugin_declaration.version, "1.0.0");
        assert!(_plugin_declaration.is_compatible());
    }

    #[test]
    fn test_source_validation() {
        let stage = HttpStage::new("http".to_string(), StageType::Source);
        let mut config = RHashMap::new();

        // Missing URL should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // With URL should succeed
        config.insert(
            RString::from("url"),
            RString::from("https://api.example.com"),
        );
        assert!(stage.validate_config(config.clone()).is_ok());

        // Invalid method should fail
        config.insert(RString::from("method"), RString::from("INVALID"));
        assert!(stage.validate_config(config).is_err());
    }

    #[test]
    fn test_sink_validation() {
        let stage = HttpStage::new("http".to_string(), StageType::Sink);
        let mut config = RHashMap::new();

        // Missing URL should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // With URL should succeed
        config.insert(
            RString::from("url"),
            RString::from("https://api.example.com"),
        );
        assert!(stage.validate_config(config.clone()).is_ok());

        // GET method should fail for sink
        config.insert(RString::from("method"), RString::from("GET"));
        assert!(stage.validate_config(config).is_err());
    }

    #[test]
    fn test_capabilities() {
        let caps = get_capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].name.as_str(), "http");
        assert_eq!(caps[0].stage_type, StageType::Source);
        assert_eq!(caps[1].stage_type, StageType::Sink);
    }
}
