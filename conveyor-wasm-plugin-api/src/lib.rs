//! Conveyor WASM Plugin API
//!
//! This crate provides the guest-side API for building Conveyor plugins as WebAssembly components.

// Generate bindings from WIT file
wit_bindgen::generate!({
    world: "plugin",
    path: "../conveyor-wasm-plugin-api/wit",
    exports: {
        world: __MyExports,
    },
});

// Re-export the generated types and export macro
pub use __MyExports as Exports;

/// Plugin API version
pub const PLUGIN_API_VERSION: u32 = 1;

// ============================================================================
// Helper Functions
// ============================================================================

/// Get config value from list of tuples
pub fn get_config_value<'a>(config: &'a [(String, String)], key: &str) -> Option<&'a str> {
    config
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Create a config error
pub fn config_error(msg: impl Into<String>) -> PluginError {
    PluginError::ConfigError(msg.into())
}

/// Create a runtime error
pub fn runtime_error(msg: impl Into<String>) -> PluginError {
    PluginError::RuntimeError(msg.into())
}

/// Create an I/O error
pub fn io_error(msg: impl Into<String>) -> PluginError {
    PluginError::IoError(msg.into())
}

/// Create a serialization error
pub fn serialization_error(msg: impl Into<String>) -> PluginError {
    PluginError::SerializationError(msg.into())
}

/// Create DataFormat from JSON records
pub fn data_format_from_json<T: serde::Serialize>(
    records: &T,
) -> Result<DataFormat, PluginError> {
    match serde_json::to_vec(records) {
        Ok(bytes) => Ok(DataFormat::JsonRecords(bytes)),
        Err(e) => Err(serialization_error(format!("Failed to serialize JSON: {}", e))),
    }
}

/// Create DataFormat from raw bytes
pub fn data_format_from_raw(bytes: Vec<u8>) -> DataFormat {
    DataFormat::Raw(bytes)
}

/// Create DataFormat from Arrow IPC bytes
pub fn data_format_from_arrow_ipc(bytes: Vec<u8>) -> DataFormat {
    DataFormat::ArrowIpc(bytes)
}

/// Extract JSON records from DataFormat
pub fn data_format_to_json<T: serde::de::DeserializeOwned>(
    data: &DataFormat,
) -> Result<T, PluginError> {
    let bytes = match data {
        DataFormat::JsonRecords(b) => b,
        DataFormat::Raw(b) => b,
        DataFormat::ArrowIpc(_) => {
            return Err(runtime_error(
                "Cannot convert ArrowIpc to JSON directly",
            ))
        }
    };

    serde_json::from_slice(bytes)
        .map_err(|e| serialization_error(format!("Failed to deserialize JSON: {}", e)))
}

/// Extract raw bytes from DataFormat
pub fn data_format_to_bytes(data: &DataFormat) -> &[u8] {
    match data {
        DataFormat::ArrowIpc(b) => b,
        DataFormat::JsonRecords(b) => b,
        DataFormat::Raw(b) => b,
    }
}

/// Helper macro for not-supported methods
#[macro_export]
macro_rules! not_supported {
    ($method:expr) => {
        Err($crate::runtime_error(format!(
            "{} is not supported by this plugin",
            $method
        )))
    };
}
