//! Test plugin for Conveyor
//!
//! A minimal plugin to verify the FFI plugin system works correctly.

use conveyor_plugin_api::{
    data::FfiDataFormat,
    rstr,
    traits::FfiDataSource,
    PluginDeclaration, RBoxError, RHashMap, RResult, RStr, RString, ROk,
};

/// Simple test data source
struct TestSource;

impl FfiDataSource for TestSource {
    fn name(&self) -> RStr<'_> {
        "test".into()
    }

    fn read(&self, _config: RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
        // Return simple test data
        let data = b"Hello from test plugin!".to_vec();
        ROk(FfiDataFormat::from_raw(data))
    }

    fn validate_config(&self, _config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        ROk(())
    }
}

/// Plugin registration function
extern "C" fn register() -> RResult<(), RBoxError> {
    // For now, just return success
    // In a full implementation, this would register the modules with the host
    ROk(())
}

/// Plugin declaration export
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: conveyor_plugin_api::PLUGIN_API_VERSION,
    name: rstr!("test"),
    version: rstr!("0.1.0"),
    description: rstr!("Simple test plugin"),
    register,
};
