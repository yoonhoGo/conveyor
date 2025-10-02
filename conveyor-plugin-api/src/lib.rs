//! Conveyor Plugin API
//!
//! This crate defines the FFI-safe plugin interface for Conveyor ETL pipelines.
//! Uses abi_stable for cross-compiler compatibility.

pub mod data;

use serde::{Deserialize, Serialize};

// Re-export abi_stable types for convenience
pub use abi_stable::{
    marker_type::ErasedObject,
    sabi_trait,
    std_types::{RBox, RBoxError, RErr, ROk, ROption, RResult, RStr, RString, RVec},
    StableAbi,
};

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
    pub fn new(name: impl Into<String>, version: impl Into<String>, description: impl Into<String>) -> Self {
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
#[repr(C)]
#[derive(StableAbi)]
pub struct PluginDeclaration {
    /// Plugin API version this plugin was compiled with
    pub api_version: u32,

    /// Plugin name
    pub name: RString,

    /// Plugin version
    pub version: RString,

    /// Plugin description
    pub description: RString,

    /// Function to register plugin modules with the host
    /// This will be called by the host after loading the plugin
    pub register: extern "C" fn() -> RResult<(), RBoxError>,
}

impl PluginDeclaration {
    /// Create a new plugin declaration
    pub const fn new(
        name: RString,
        version: RString,
        description: RString,
        register: extern "C" fn() -> RResult<(), RBoxError>,
    ) -> Self {
        Self {
            api_version: PLUGIN_API_VERSION,
            name,
            version,
            description,
            register,
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
        extern "C" fn dummy_register() -> RResult<(), RBoxError> {
            ROk(())
        }

        let decl = PluginDeclaration::new(
            RString::from("test"),
            RString::from("1.0.0"),
            RString::from("Test plugin"),
            dummy_register,
        );

        assert_eq!(decl.api_version, PLUGIN_API_VERSION);
        assert!(decl.is_compatible());
        assert_eq!(decl.name.as_str(), "test");
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
