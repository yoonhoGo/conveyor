//! FFI-safe plugin trait definitions
//!
//! This module defines the core FfiStage trait that plugins must implement.
//! All traits are FFI-safe using the #[sabi_trait] attribute.

use crate::{data::FfiDataFormat, sabi_trait, RBoxError, RHashMap, RResult, RStr, RString};

/// FFI-safe execution context
///
/// Contains the input data and configuration for a stage execution.
#[repr(C)]
#[derive(crate::StableAbi, Debug, Clone)]
pub struct FfiExecutionContext {
    /// Input data from previous stages
    /// Key: input stage ID, Value: data from that stage
    pub inputs: RHashMap<RString, FfiDataFormat>,

    /// Configuration parameters
    pub config: RHashMap<RString, RString>,
}

impl FfiExecutionContext {
    /// Create a new execution context
    pub fn new(
        inputs: RHashMap<RString, FfiDataFormat>,
        config: RHashMap<RString, RString>,
    ) -> Self {
        Self { inputs, config }
    }

    /// Create an empty context
    pub fn empty() -> Self {
        Self {
            inputs: RHashMap::new(),
            config: RHashMap::new(),
        }
    }

    /// Get input data by ID
    pub fn get_input(&self, id: &str) -> Option<&FfiDataFormat> {
        self.inputs.get(&RString::from(id))
    }

    /// Get configuration value by key
    pub fn get_config(&self, key: &str) -> Option<&RString> {
        self.config.get(&RString::from(key))
    }
}

/// Stage type enumeration
#[repr(C)]
#[derive(crate::StableAbi, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageType {
    /// Source stage (reads data from external sources)
    Source,
    /// Transform stage (transforms data)
    Transform,
    /// Sink stage (writes data to external destinations)
    Sink,
}

impl StageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StageType::Source => "source",
            StageType::Transform => "transform",
            StageType::Sink => "sink",
        }
    }
}

/// FFI-safe Stage trait
///
/// This is the unified interface for all pipeline stages (sources, transforms, sinks).
/// Plugins implement this trait to provide custom pipeline stages.
///
/// # Input-Aware Execution
///
/// The `execute` method receives a context with:
/// - `inputs`: Data from previous stages in the DAG
/// - `config`: Configuration parameters from TOML
///
/// This allows stages to:
/// - Use previous stage data to make dynamic decisions (e.g., HTTP fetch)
/// - Combine multiple inputs (e.g., join operation)
/// - Execute conditionally based on input data
#[sabi_trait]
pub trait FfiStage: Send + Sync {
    /// Get the name of this stage
    fn name(&self) -> RStr<'_>;

    /// Get the type of this stage
    fn stage_type(&self) -> StageType;

    /// Execute the stage
    ///
    /// # Arguments
    /// * `context` - Execution context with inputs and configuration
    ///
    /// # Returns
    /// * `RResult<FfiDataFormat, RBoxError>` - Output data or error
    ///
    /// # Examples
    ///
    /// **Source Stage (no inputs):**
    /// ```ignore
    /// fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
    ///     // context.inputs is empty for sources
    ///     let url = context.get_config("url").unwrap();
    ///     // Fetch and return data
    /// }
    /// ```
    ///
    /// **Transform Stage (single input):**
    /// ```ignore
    /// fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
    ///     let input = context.inputs.values().next().unwrap();
    ///     // Transform and return data
    /// }
    /// ```
    ///
    /// **Input-Aware Transform (uses input data in logic):**
    /// ```ignore
    /// fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
    ///     let input = context.inputs.values().next().unwrap();
    ///     // Parse input data
    ///     let records = input.to_json_records()?;
    ///
    ///     // Use input data to make API calls
    ///     for record in records {
    ///         let id = record.get("id").unwrap();
    ///         let url = format!("https://api.example.com/users/{}", id);
    ///         // Fetch related data...
    ///     }
    /// }
    /// ```
    ///
    /// **Sink Stage (consumes input):**
    /// ```ignore
    /// fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
    ///     let input = context.inputs.values().next().unwrap();
    ///     // Write data to destination
    ///     // Return empty or confirmation data
    /// }
    /// ```
    fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError>;

    /// Validate the configuration
    ///
    /// # Arguments
    /// * `config` - Configuration parameters to validate
    ///
    /// # Returns
    /// * `RResult<(), RBoxError>` - Success or validation error
    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError>;
}

/// Stage factory function type
///
/// This function creates a new instance of a stage.
/// It returns a trait object that implements FfiStage.
///
/// Plugins should export factory functions with `#[no_mangle]`:
///
/// ```rust,ignore
/// #[no_mangle]
/// pub extern "C" fn create_http_source() -> FfiStage_TO<'static, RBox<()>> {
///     FfiStage_TO::from_value(HttpSource::new(), TD_Opaque)
/// }
/// ```
pub type StageFactory = extern "C" fn() -> FfiStage_TO<'static, crate::RBox<()>>;

/// Plugin capability information
///
/// Describes a stage that the plugin provides.
///
/// The plugin exports a factory function for each stage, and the host loads it by symbol name.
#[repr(C)]
#[derive(crate::StableAbi, Debug, Clone)]
pub struct PluginCapability {
    /// Stage name (e.g., "http", "mongodb")
    pub name: RString,

    /// Stage type
    pub stage_type: StageType,

    /// Description of what this stage does
    pub description: RString,

    /// Factory function symbol name (e.g., "create_http_source")
    ///
    /// The host will use `libloading` to load this symbol from the plugin library.
    pub factory_symbol: RString,
}

impl PluginCapability {
    pub fn new(
        name: impl Into<RString>,
        stage_type: StageType,
        description: impl Into<RString>,
        factory_symbol: impl Into<RString>,
    ) -> Self {
        Self {
            name: name.into(),
            stage_type,
            description: description.into(),
            factory_symbol: factory_symbol.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RBox, ROk};

    // Test implementation of FfiStage
    struct TestStage {
        name: String,
        stage_type: StageType,
    }

    impl FfiStage for TestStage {
        fn name(&self) -> RStr<'_> {
            self.name.as_str().into()
        }

        fn stage_type(&self) -> StageType {
            self.stage_type
        }

        fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
            match self.stage_type {
                StageType::Source => {
                    // Source: generate data
                    ROk(FfiDataFormat::from_raw(vec![1, 2, 3]))
                }
                StageType::Transform => {
                    // Transform: pass through first input
                    if let Some(input) = context.inputs.values().next() {
                        ROk(input.clone())
                    } else {
                        ROk(FfiDataFormat::from_raw(vec![]))
                    }
                }
                StageType::Sink => {
                    // Sink: consume data and return empty
                    ROk(FfiDataFormat::from_raw(vec![]))
                }
            }
        }

        fn validate_config(&self, _config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
            ROk(())
        }
    }

    #[test]
    fn test_execution_context() {
        let mut inputs = RHashMap::new();
        inputs.insert(
            RString::from("input1"),
            FfiDataFormat::from_raw(vec![1, 2, 3]),
        );

        let mut config = RHashMap::new();
        config.insert(RString::from("key"), RString::from("value"));

        let context = FfiExecutionContext::new(inputs, config);

        assert!(context.get_input("input1").is_some());
        assert!(context.get_input("nonexistent").is_none());
        assert_eq!(context.get_config("key").unwrap().as_str(), "value");
    }

    #[test]
    fn test_empty_context() {
        let context = FfiExecutionContext::empty();
        assert!(context.inputs.is_empty());
        assert!(context.config.is_empty());
    }

    #[test]
    fn test_stage_type() {
        assert_eq!(StageType::Source.as_str(), "source");
        assert_eq!(StageType::Transform.as_str(), "transform");
        assert_eq!(StageType::Sink.as_str(), "sink");
    }

    #[test]
    fn test_source_stage() {
        let stage = TestStage {
            name: "test_source".to_string(),
            stage_type: StageType::Source,
        };

        assert_eq!(stage.name(), "test_source");
        assert_eq!(stage.stage_type(), StageType::Source);

        let context = FfiExecutionContext::empty();
        let result = stage.execute(context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform_stage() {
        let stage = TestStage {
            name: "test_transform".to_string(),
            stage_type: StageType::Transform,
        };

        assert_eq!(stage.stage_type(), StageType::Transform);

        let mut inputs = RHashMap::new();
        inputs.insert(
            RString::from("input1"),
            FfiDataFormat::from_raw(vec![1, 2, 3]),
        );

        let context = FfiExecutionContext::new(inputs, RHashMap::new());
        let result = stage.execute(context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sink_stage() {
        let stage = TestStage {
            name: "test_sink".to_string(),
            stage_type: StageType::Sink,
        };

        assert_eq!(stage.stage_type(), StageType::Sink);

        let context = FfiExecutionContext::empty();
        let result = stage.execute(context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_plugin_capability() {
        let cap = PluginCapability::new(
            "http",
            StageType::Source,
            "HTTP data source",
            "create_http_source",
        );

        assert_eq!(cap.name.as_str(), "http");
        assert_eq!(cap.stage_type, StageType::Source);
        assert_eq!(cap.description.as_str(), "HTTP data source");
        assert_eq!(cap.factory_symbol.as_str(), "create_http_source");
    }

    #[test]
    fn test_trait_object() {
        use crate::sabi_trait::prelude::*;

        let stage = TestStage {
            name: "test".to_string(),
            stage_type: StageType::Source,
        };

        // Test that we can create a trait object
        let stage_to: FfiStage_TO<'_, RBox<()>> = FfiStage_TO::from_value(stage, TD_Opaque);
        assert_eq!(stage_to.name(), "test");
        assert_eq!(stage_to.stage_type(), StageType::Source);
    }
}
