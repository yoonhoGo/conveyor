//! MongoDB Plugin for Conveyor
//!
//! Provides ABI-stable MongoDB source and sink capabilities.

use abi_stable::std_types::RString;
use conveyor_plugin_api::*;

// TODO: Implement MongoDB source and sink with actual MongoDB logic

pub struct MongoDbSource;

impl SourcePlugin for MongoDbSource {
    fn name(&self) -> RString {
        "mongodb".into()
    }

    fn read(&self, _config: PluginConfig) -> PluginResult<DataFormat> {
        Err(PluginError::new("MongoDB source not yet implemented"))
    }

    fn validate_config(&self, config: &PluginConfig) -> PluginResult<()> {
        if !config.contains_key(&"uri".into()) {
            return Err(PluginError::new(
                "MongoDB source requires 'uri' configuration",
            ));
        }
        Ok(())
    }
}

pub struct MongoDbSink;

impl SinkPlugin for MongoDbSink {
    fn name(&self) -> RString {
        "mongodb".into()
    }

    fn write(&self, _data: DataFormat, _config: PluginConfig) -> PluginResult<()> {
        Err(PluginError::new("MongoDB sink not yet implemented"))
    }

    fn validate_config(&self, config: &PluginConfig) -> PluginResult<()> {
        if !config.contains_key(&"uri".into()) {
            return Err(PluginError::new(
                "MongoDB sink requires 'uri' configuration",
            ));
        }
        Ok(())
    }
}

// Export plugin using the macro
conveyor_plugin_api::export_plugin! {
    name: "mongodb",
    version: env!("CARGO_PKG_VERSION"),
    description: "MongoDB plugin for database integration",
    source: MongoDbSource,
    sink: MongoDbSink,
}
