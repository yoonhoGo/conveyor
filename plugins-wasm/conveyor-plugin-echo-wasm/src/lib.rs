//! Echo WASM Plugin for Conveyor - Version 2 (Unified API)
//!
//! A simple test plugin that echoes data back, useful for testing the WASM plugin system.
//! Supports source, transform, and sink operations.

use std::collections::HashMap;

// Generate WIT bindings
wit_bindgen::generate!({
    world: "plugin",
    path: "../../conveyor-wasm-plugin-api/wit",
});

// Plugin API version
const PLUGIN_API_VERSION: u32 = 1;

// Helper functions
fn get_config_value<'a>(config: &'a [(String, String)], key: &str) -> Option<&'a str> {
    config
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn data_format_from_json<T: serde::Serialize>(
    records: &T,
) -> Result<DataFormat, PluginError> {
    match serde_json::to_vec(records) {
        Ok(bytes) => Ok(DataFormat::JsonRecords(bytes)),
        Err(e) => Err(PluginError::SerializationError(format!("Failed to serialize JSON: {}", e))),
    }
}

fn data_format_to_bytes(data: &DataFormat) -> &[u8] {
    match data {
        DataFormat::ArrowIpc(b) => b,
        DataFormat::JsonRecords(b) => b,
        DataFormat::Raw(b) => b,
    }
}

/// Echo plugin structure
struct EchoPlugin;

impl Guest for EchoPlugin {
    /// Get plugin metadata
    fn get_metadata() -> PluginMetadata {
        PluginMetadata {
            name: "echo-wasm".to_string(),
            version: "1.0.0".to_string(),
            description: "Echo WASM plugin for testing".to_string(),
            api_version: PLUGIN_API_VERSION,
        }
    }

    /// Get list of stages this plugin provides
    fn get_capabilities() -> Vec<StageCapability> {
        vec![
            StageCapability {
                name: "echo".to_string(),
                stage_type: StageType::Source,
                description: "Echo source - returns a test message".to_string(),
            },
            StageCapability {
                name: "echo".to_string(),
                stage_type: StageType::Transform,
                description: "Echo transform - passes data through unchanged".to_string(),
            },
            StageCapability {
                name: "echo".to_string(),
                stage_type: StageType::Sink,
                description: "Echo sink - validates and accepts data".to_string(),
            },
        ]
    }

    /// Execute a stage
    fn execute(stage_name: String, context: ExecutionContext) -> Result<DataFormat, PluginError> {
        // Verify stage name
        if stage_name != "echo" {
            return Err(PluginError::ConfigError(format!(
                "Unknown stage: {}. Echo plugin only provides 'echo' stage",
                stage_name
            )));
        }

        // Determine operation type based on inputs
        if context.inputs.is_empty() {
            // No inputs = Source operation
            execute_source(&context.config)
        } else {
            // Has inputs - check if there's a "mode" config
            let mode = get_config_value(&context.config, "mode").unwrap_or("transform");

            match mode {
                "sink" => execute_sink(&context.inputs, &context.config),
                _ => execute_transform(&context.inputs, &context.config),
            }
        }
    }

    /// Validate configuration for a stage
    fn validate_config(stage_name: String, _config: Vec<(String, String)>) -> Result<(), PluginError> {
        // Verify stage name
        if stage_name != "echo" {
            return Err(PluginError::ConfigError(format!(
                "Unknown stage: {}. Echo plugin only provides 'echo' stage",
                stage_name
            )));
        }

        // Echo plugin accepts any configuration
        Ok(())
    }
}

/// Execute as source (generate test data)
fn execute_source(config: &[(String, String)]) -> Result<DataFormat, PluginError> {
    // Get message from config, or use default
    let message = get_config_value(config, "message")
        .unwrap_or("Hello from WASM Echo Plugin!");

    // Create a simple JSON record
    let mut record = HashMap::new();
    record.insert("message".to_string(), serde_json::Value::String(message.to_string()));
    record.insert("plugin".to_string(), serde_json::Value::String("echo-wasm".to_string()));
    record.insert("type".to_string(), serde_json::Value::String("source".to_string()));

    let records = vec![record];
    data_format_from_json(&records)
}

/// Execute as transform (pass data through)
fn execute_transform(
    inputs: &[(String, DataFormat)],
    config: &[(String, String)],
) -> Result<DataFormat, PluginError> {
    // Get the first input (echo transform works on single input)
    let input_data = inputs
        .first()
        .ok_or_else(|| PluginError::RuntimeError("Transform requires at least one input".to_string()))?;

    // Check if we should add metadata
    let add_metadata = get_config_value(config, "add_metadata")
        .map(|v| v == "true")
        .unwrap_or(false);

    if add_metadata {
        // Parse input data and add echo metadata
        let bytes = data_format_to_bytes(&input_data.1);

        match &input_data.1 {
            DataFormat::JsonRecords(_) => {
                // Parse and add metadata
                let mut records: Vec<HashMap<String, serde_json::Value>> =
                    serde_json::from_slice(bytes)
                        .map_err(|e| PluginError::SerializationError(format!("Failed to parse JSON: {}", e)))?;

                // Add echo metadata to each record
                for record in &mut records {
                    record.insert("_echo_processed".to_string(), serde_json::Value::Bool(true));
                }

                data_format_from_json(&records)
            }
            _ => {
                // For other formats, just pass through
                Ok(input_data.1.clone())
            }
        }
    } else {
        // Just pass through unchanged
        Ok(input_data.1.clone())
    }
}

/// Execute as sink (validate and accept data)
fn execute_sink(
    inputs: &[(String, DataFormat)],
    _config: &[(String, String)],
) -> Result<DataFormat, PluginError> {
    // Get the first input
    let input_data = inputs
        .first()
        .ok_or_else(|| PluginError::RuntimeError("Sink requires at least one input".to_string()))?;

    // Validate we can read it
    let bytes = data_format_to_bytes(&input_data.1);

    if bytes.is_empty() {
        return Err(PluginError::RuntimeError("Received empty data".to_string()));
    }

    // Return the input data (sinks pass through)
    Ok(input_data.1.clone())
}

// Export the plugin implementation
export!(EchoPlugin);
