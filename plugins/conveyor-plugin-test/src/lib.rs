//! Test plugin for Conveyor - Version 2 (Unified Stage API)
//!
//! A minimal plugin to verify the FFI plugin system works correctly.

use conveyor_plugin_api::sabi_trait::prelude::*;
use conveyor_plugin_api::traits::{FfiExecutionContext, FfiStage, FfiStage_TO};
use conveyor_plugin_api::{
    rstr, FfiDataFormat, PluginCapability, PluginDeclaration, RBox, RBoxError, RErr, RHashMap, ROk,
    RResult, RString, RVec, StageType, PLUGIN_API_VERSION,
};

/// Simple test stage
pub struct TestStage {
    name: String,
    stage_type: StageType,
}

impl TestStage {
    fn new(name: String, stage_type: StageType) -> Self {
        Self { name, stage_type }
    }
}

impl FfiStage for TestStage {
    fn name(&self) -> conveyor_plugin_api::RStr<'_> {
        self.name.as_str().into()
    }

    fn stage_type(&self) -> StageType {
        self.stage_type
    }

    fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
        match self.stage_type {
            StageType::Source => {
                // Return simple test data
                let data = b"Hello from test plugin!".to_vec();
                ROk(FfiDataFormat::from_raw(data))
            }
            StageType::Transform => {
                // Pass through first input
                let input_data = match context.inputs.into_iter().next() {
                    Some(tuple) => tuple.1,
                    None => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Test transform requires input data"
                        )))
                    }
                };
                ROk(input_data)
            }
            StageType::Sink => {
                // Validate input exists
                let input_data = match context.inputs.into_iter().next() {
                    Some(tuple) => tuple.1,
                    None => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "Test sink requires input data"
                        )))
                    }
                };
                ROk(input_data)
            }
        }
    }

    fn validate_config(&self, _config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        ROk(())
    }
}

// Factory functions
#[no_mangle]
pub extern "C" fn create_test_source() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        TestStage::new("test".to_string(), StageType::Source),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_test_transform() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        TestStage::new("test".to_string(), StageType::Transform),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_test_sink() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        TestStage::new("test".to_string(), StageType::Sink),
        TD_Opaque,
    )
}

// Plugin capabilities
extern "C" fn get_capabilities() -> RVec<PluginCapability> {
    vec![
        PluginCapability {
            name: RString::from("test"),
            stage_type: StageType::Source,
            description: RString::from("Test source - returns simple test data"),
            factory_symbol: RString::from("create_test_source"),
        },
        PluginCapability {
            name: RString::from("test"),
            stage_type: StageType::Transform,
            description: RString::from("Test transform - passes data through"),
            factory_symbol: RString::from("create_test_transform"),
        },
        PluginCapability {
            name: RString::from("test"),
            stage_type: StageType::Sink,
            description: RString::from("Test sink - validates data"),
            factory_symbol: RString::from("create_test_sink"),
        },
    ]
    .into()
}

/// Plugin declaration export
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("test"),
    version: rstr!("1.0.0"),
    description: rstr!("Simple test plugin"),
    get_capabilities,
};
