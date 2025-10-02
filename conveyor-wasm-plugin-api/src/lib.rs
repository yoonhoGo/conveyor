//! Conveyor WASM Plugin API (Version 2)
//!
//! This crate provides the guest-side API for building Conveyor plugins as WebAssembly components.
//!
//! # Architecture
//!
//! Version 2 introduces a unified `execute` interface that supports input-aware execution.
//! All stages (sources, transforms, sinks) use the same execution model:
//!
//! - **Sources**: `context.inputs` is empty, use `context.config` to fetch data
//! - **Transforms**: `context.inputs` contains data from previous stages
//! - **Sinks**: `context.inputs` contains data to write
//!
//! # Example
//!
//! ```rust,ignore
//! use conveyor_wasm_plugin_api::*;
//!
//! struct MyPlugin;
//!
//! impl Exports for MyPlugin {
//!     fn get_metadata() -> PluginMetadata {
//!         PluginMetadata {
//!             name: "my_plugin".to_string(),
//!             version: "1.0.0".to_string(),
//!             description: "My custom plugin".to_string(),
//!             api_version: PLUGIN_API_VERSION,
//!         }
//!     }
//!
//!     fn get_capabilities() -> Vec<StageCapability> {
//!         vec![
//!             StageCapability {
//!                 name: "http".to_string(),
//!                 stage_type: StageType::Source,
//!                 description: "HTTP data source".to_string(),
//!             },
//!         ]
//!     }
//!
//!     fn execute(stage_name: String, context: ExecutionContext) -> Result<DataFormat, PluginError> {
//!         match stage_name.as_str() {
//!             "http" => {
//!                 let url = get_config_value(&context.config, "url")
//!                     .ok_or_else(|| config_error("Missing 'url' config"))?;
//!                 // Fetch data from URL...
//!                 Ok(data_format_from_json(&data)?)
//!             }
//!             _ => Err(runtime_error(format!("Unknown stage: {}", stage_name)))
//!         }
//!     }
//!
//!     fn validate_config(stage_name: String, config: Vec<(String, String)>) -> Result<(), PluginError> {
//!         match stage_name.as_str() {
//!             "http" => {
//!                 if get_config_value(&config, "url").is_none() {
//!                     return Err(config_error("Missing 'url' config"));
//!                 }
//!                 Ok(())
//!             }
//!             _ => Err(runtime_error(format!("Unknown stage: {}", stage_name)))
//!         }
//!     }
//! }
//! ```

// Generate bindings from WIT file
wit_bindgen::generate!({
    world: "plugin",
});

// Re-export the generated types for convenience
// The wit-bindgen macro generates a `Guest` trait and related types
pub use Guest as PluginInterface;

/// Plugin API version (Version 2: Unified execution interface)
pub const PLUGIN_API_VERSION: u32 = 2;

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

/// Get input data by stage ID from execution context
pub fn get_input<'a>(context: &'a ExecutionContext, id: &str) -> Option<&'a DataFormat> {
    context
        .inputs
        .iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| v)
}

/// Get the first input from execution context (for single-input transforms)
pub fn get_first_input(context: &ExecutionContext) -> Option<&DataFormat> {
    context.inputs.first().map(|(_, v)| v)
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

/// Create empty DataFormat (for sinks that don't return data)
pub fn data_format_empty() -> DataFormat {
    DataFormat::Raw(vec![])
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

/// Helper to create a source stage capability
pub fn source_capability(name: impl Into<String>, description: impl Into<String>) -> StageCapability {
    StageCapability {
        name: name.into(),
        stage_type: StageType::Source,
        description: description.into(),
    }
}

/// Helper to create a transform stage capability
pub fn transform_capability(name: impl Into<String>, description: impl Into<String>) -> StageCapability {
    StageCapability {
        name: name.into(),
        stage_type: StageType::Transform,
        description: description.into(),
    }
}

/// Helper to create a sink stage capability
pub fn sink_capability(name: impl Into<String>, description: impl Into<String>) -> StageCapability {
    StageCapability {
        name: name.into(),
        stage_type: StageType::Sink,
        description: description.into(),
    }
}

/// Helper macro for not-supported stages
#[macro_export]
macro_rules! not_supported {
    ($stage:expr) => {
        Err($crate::runtime_error(format!(
            "Stage '{}' is not supported by this plugin",
            $stage
        )))
    };
}
