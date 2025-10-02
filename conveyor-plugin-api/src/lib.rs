//! Conveyor Plugin API
//!
//! This crate defines the plugin interface for Conveyor ETL pipelines.
//! Currently provides version management for plugin compatibility.

use serde::{Deserialize, Serialize};

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
}
