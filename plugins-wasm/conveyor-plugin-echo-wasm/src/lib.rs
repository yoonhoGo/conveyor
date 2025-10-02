//! Echo WASM Plugin for Conveyor
//!
//! A simple test plugin that echoes data back, useful for testing the WASM plugin system.

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
            version: "0.1.0".to_string(),
            description: "Echo WASM plugin for testing".to_string(),
            api_version: PLUGIN_API_VERSION,
        }
    }

    /// Read operation - returns a simple message
    fn read(config: Vec<(String, String)>) -> Result<DataFormat, PluginError> {
        // Get message from config, or use default
        let message = get_config_value(&config, "message")
            .unwrap_or("Hello from WASM Echo Plugin!");

        // Create a simple JSON record
        let mut record = HashMap::new();
        record.insert("message".to_string(), serde_json::Value::String(message.to_string()));
        record.insert("plugin".to_string(), serde_json::Value::String("echo-wasm".to_string()));

        let records = vec![record];
        data_format_from_json(&records)
    }

    /// Write operation - accepts any data and returns success
    fn write(data: DataFormat, _config: Vec<(String, String)>) -> Result<(), PluginError> {
        // In a real plugin, we would write data somewhere
        // For echo plugin, we just validate we can read it
        let bytes = data_format_to_bytes(&data);

        if bytes.is_empty() {
            return Err(PluginError::RuntimeError("Received empty data".to_string()));
        }

        // Successfully "wrote" the data (actually just validated it)
        Ok(())
    }

    /// Transform operation - passes data through unchanged
    fn transform(data: DataFormat, _config: Vec<(String, String)>) -> Result<DataFormat, PluginError> {
        // Echo plugin just returns the data as-is
        Ok(data)
    }

    /// Validate configuration
    fn validate_config(_config: Vec<(String, String)>) -> Result<(), PluginError> {
        // Echo plugin accepts any configuration
        Ok(())
    }
}

// Export the plugin implementation
export!(EchoPlugin);
