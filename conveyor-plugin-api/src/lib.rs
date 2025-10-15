//! Conveyor Plugin API
//!
//! This crate defines the FFI-safe plugin interface for Conveyor ETL pipelines.
//! Uses abi_stable for cross-compiler compatibility.
//!
//! # Architecture
//!
//! Plugins implement the `FfiStage` trait to provide custom pipeline stages.
//! The unified `FfiStage` interface supports:
//!
//! - **Sources**: Generate or fetch data from external sources
//! - **Transforms**: Process and transform data
//! - **Sinks**: Write data to external destinations
//!
//! All stages are **input-aware**, meaning they can access data from previous
//! stages in the pipeline DAG. This enables advanced use cases like:
//!
//! - HTTP fetch transforms that use input data to construct API requests
//! - Join operations that combine multiple inputs
//! - Conditional execution based on input data
//!
//! # Example
//!
//! ```rust,ignore
//! use conveyor_plugin_api::*;
//!
//! struct MyHttpStage;
//!
//! impl FfiStage for MyHttpStage {
//!     fn name(&self) -> RStr<'_> {
//!         "http".into()
//!     }
//!
//!     fn stage_type(&self) -> StageType {
//!         StageType::Source
//!     }
//!
//!     fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
//!         let url = context.get_config("url").unwrap();
//!         // Fetch data from URL...
//!         ROk(FfiDataFormat::from_json_records(&data)?)
//!     }
//!
//!     fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
//!         if !config.contains_key(&RString::from("url")) {
//!             return RErr(RBoxError::from_fmt(&format_args!("Missing 'url' config")));
//!         }
//!         ROk(())
//!     }
//! }
//! ```

pub mod data;
pub mod metadata;
pub mod traits;

use serde::{Deserialize, Serialize};

// Re-export abi_stable types for convenience
pub use abi_stable::{
    marker_type::ErasedObject,
    rstr, sabi_trait,
    std_types::{RBox, RBoxError, RErr, RHashMap, ROk, ROption, RResult, RStr, RString, RVec},
    StableAbi,
};

// Re-export core trait types
pub use data::FfiDataFormat;
pub use metadata::{FfiConfigParameter, FfiParameterType, FfiStageMetadata};
pub use traits::{FfiExecutionContext, FfiStage, PluginCapability, StageType};

/// Plugin API version - increment when breaking changes occur
///
/// This version is used to ensure compatibility between the host application
/// and dynamically loaded plugins. Plugins compiled with a different API version
/// will be rejected during loading.
pub const PLUGIN_API_VERSION: u32 = 1;

/// Plugin metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub api_version: u32,
}

impl PluginMetadata {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: description.into(),
            api_version: PLUGIN_API_VERSION,
        }
    }

    pub fn is_compatible(&self) -> bool {
        self.api_version == PLUGIN_API_VERSION
    }
}

/// FFI-safe plugin declaration
///
/// This struct is the entry point for a plugin. It should be exported with
/// the name `_plugin_declaration` from the plugin's dynamic library.
///
/// # Example
///
/// ```rust,ignore
/// use conveyor_plugin_api::*;
///
/// #[no_mangle]
/// pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
///     api_version: PLUGIN_API_VERSION,
///     name: rstr!("my_plugin"),
///     version: rstr!("1.0.0"),
///     description: rstr!("My custom plugin"),
///     get_capabilities,
/// };
///
/// extern "C" fn get_capabilities() -> RVec<PluginCapability> {
///     vec![
///         PluginCapability::new("http", StageType::Source, "HTTP data source"),
///     ].into()
/// }
/// ```
#[repr(C)]
#[derive(StableAbi)]
pub struct PluginDeclaration {
    /// Plugin API version this plugin was compiled with
    pub api_version: u32,

    /// Plugin name (static string slice)
    pub name: RStr<'static>,

    /// Plugin version (static string slice)
    pub version: RStr<'static>,

    /// Plugin description (static string slice)
    pub description: RStr<'static>,

    /// Function to get plugin capabilities
    ///
    /// Returns a list of stages that this plugin provides.
    /// The host will use this information to know what stages are available.
    pub get_capabilities: extern "C" fn() -> RVec<PluginCapability>,
}

impl PluginDeclaration {
    /// Create a new plugin declaration
    pub const fn new(
        name: RStr<'static>,
        version: RStr<'static>,
        description: RStr<'static>,
        get_capabilities: extern "C" fn() -> RVec<PluginCapability>,
    ) -> Self {
        Self {
            api_version: PLUGIN_API_VERSION,
            name,
            version,
            description,
            get_capabilities,
        }
    }

    /// Check if this plugin is compatible with the host
    pub fn is_compatible(&self) -> bool {
        self.api_version == PLUGIN_API_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata_compatibility() {
        let metadata = PluginMetadata::new("test", "1.0.0", "Test plugin");
        assert_eq!(metadata.api_version, PLUGIN_API_VERSION);
        assert!(metadata.is_compatible());
    }

    #[test]
    fn test_incompatible_version() {
        let mut metadata = PluginMetadata::new("test", "1.0.0", "Test plugin");
        metadata.api_version = 999;
        assert!(!metadata.is_compatible());
    }

    #[test]
    fn test_plugin_declaration_compatibility() {
        extern "C" fn get_capabilities() -> RVec<PluginCapability> {
            RVec::new()
        }

        let decl = PluginDeclaration::new(
            RStr::from("test"),
            RStr::from("1.0.0"),
            RStr::from("Test plugin"),
            get_capabilities,
        );

        assert_eq!(decl.api_version, PLUGIN_API_VERSION);
        assert!(decl.is_compatible());
        assert_eq!(decl.name, "test");
    }

    #[test]
    fn test_ffi_safe_types() {
        // Test that FFI-safe types work correctly
        let rstring = RString::from("hello");
        assert_eq!(rstring.as_str(), "hello");

        let rvec: RVec<i32> = RVec::from(vec![1, 2, 3]);
        assert_eq!(rvec.len(), 3);

        let result: RResult<i32, RBoxError> = ROk(42);
        assert!(result.is_ok());
    }
}
