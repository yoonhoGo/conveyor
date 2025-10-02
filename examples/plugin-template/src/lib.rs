//! Conveyor Plugin Template
//!
//! This template demonstrates how to create a Conveyor plugin with:
//! - DataSource: Read data from a source
//! - Transform: Transform data
//! - Sink: Write data to a destination
//!
//! Replace YOURNAME with your actual plugin name throughout this file.

use conveyor_plugin_api::{
    data::FfiDataFormat,
    rstr,
    traits::{FfiDataSource, FfiSink, FfiTransform},
    PluginDeclaration, RBoxError, RHashMap, RResult, RStr, RString, ROk, RErr,
};

// ============================================================================
// Data Source Implementation
// ============================================================================

/// Your custom data source
///
/// Example: Reading from a custom API, database, or file format
struct YourNameSource;

impl FfiDataSource for YourNameSource {
    fn name(&self) -> RStr<'_> {
        "yourname_source".into()
    }

    fn read(&self, config: RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
        // TODO: Implement your data reading logic here

        // Example: Read configuration
        // let url = config.get(&RString::from("url"))
        //     .ok_or_else(|| RBoxError::from_fmt(&format_args!("Missing 'url' config")))?;

        // Example: Return sample data
        let sample_data = b"Hello from YourName plugin!".to_vec();
        ROk(FfiDataFormat::from_raw(sample_data))
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // TODO: Validate required configuration

        // Example: Check required fields
        // if !config.contains_key(&RString::from("url")) {
        //     return RErr(RBoxError::from_fmt(&format_args!("Missing required 'url' configuration")));
        // }

        ROk(())
    }
}

// ============================================================================
// Transform Implementation
// ============================================================================

/// Your custom transform
///
/// Example: Data filtering, mapping, or enrichment
struct YourNameTransform;

impl FfiTransform for YourNameTransform {
    fn name(&self) -> RStr<'_> {
        "yourname_transform".into()
    }

    fn apply(
        &self,
        data: FfiDataFormat,
        config: RHashMap<RString, RString>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        // TODO: Implement your transformation logic here

        // Example: Just pass through the data
        ROk(data)

        // Example: Transform the data
        // match data {
        //     FfiDataFormat::Raw(bytes) => {
        //         // Process raw bytes
        //         let processed = process_bytes(bytes.as_slice());
        //         ROk(FfiDataFormat::from_raw(processed))
        //     }
        //     FfiDataFormat::JsonRecords(bytes) => {
        //         // Process JSON records
        //         ROk(FfiDataFormat::JsonRecords(bytes))
        //     }
        //     FfiDataFormat::ArrowIpc(bytes) => {
        //         // Process Arrow IPC data
        //         ROk(FfiDataFormat::ArrowIpc(bytes))
        //     }
        // }
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // TODO: Validate transformation configuration
        ROk(())
    }
}

// ============================================================================
// Sink Implementation
// ============================================================================

/// Your custom sink
///
/// Example: Writing to a custom API, database, or file format
struct YourNameSink;

impl FfiSink for YourNameSink {
    fn name(&self) -> RStr<'_> {
        "yourname_sink".into()
    }

    fn write(
        &self,
        data: FfiDataFormat,
        config: RHashMap<RString, RString>,
    ) -> RResult<(), RBoxError> {
        // TODO: Implement your data writing logic here

        // Example: Get configuration
        // let output_path = config.get(&RString::from("path"))
        //     .ok_or_else(|| RBoxError::from_fmt(&format_args!("Missing 'path' config")))?;

        // Example: Write data based on format
        // match data {
        //     FfiDataFormat::Raw(bytes) => {
        //         // Write raw bytes to destination
        //         std::fs::write(output_path.as_str(), bytes.as_slice())
        //             .map_err(|e| RBoxError::from_box(Box::new(e)))?;
        //     }
        //     _ => {
        //         return RErr(RBoxError::from_fmt(&format_args!("Unsupported format")));
        //     }
        // }

        ROk(())
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // TODO: Validate sink configuration
        ROk(())
    }
}

// ============================================================================
// Plugin Registration
// ============================================================================

/// Register your plugin modules with the host
///
/// This function is called by the host after loading the plugin.
/// Register all your sources, transforms, and sinks here.
extern "C" fn register() -> RResult<(), RBoxError> {
    // TODO: Register your modules

    // In the future, this will register modules with the host's registry:
    // registry.register_source("yourname_source", Box::new(YourNameSource));
    // registry.register_transform("yourname_transform", Box::new(YourNameTransform));
    // registry.register_sink("yourname_sink", Box::new(YourNameSink));

    ROk(())
}

/// Plugin declaration - exported symbol
///
/// IMPORTANT: This static variable MUST be named `_plugin_declaration`
/// and exported with #[no_mangle] for the host to find it.
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: conveyor_plugin_api::PLUGIN_API_VERSION,

    // TODO: Update these with your plugin information
    name: rstr!("yourname"),
    version: rstr!("0.1.0"),
    description: rstr!("Your plugin description here"),

    register,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source() {
        let source = YourNameSource;
        assert_eq!(source.name(), "yourname_source");

        let config = RHashMap::new();
        let result = source.read(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform() {
        let transform = YourNameTransform;
        assert_eq!(transform.name(), "yourname_transform");

        let data = FfiDataFormat::from_raw(vec![1, 2, 3]);
        let config = RHashMap::new();
        let result = transform.apply(data, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sink() {
        let sink = YourNameSink;
        assert_eq!(sink.name(), "yourname_sink");

        let data = FfiDataFormat::from_raw(vec![1, 2, 3]);
        let config = RHashMap::new();
        let result = sink.write(data, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_plugin_declaration() {
        assert_eq!(_plugin_declaration.name, "yourname");
        assert_eq!(_plugin_declaration.version, "0.1.0");
        assert!(_plugin_declaration.is_compatible());
    }
}
