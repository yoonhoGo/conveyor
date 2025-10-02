//! FFI-safe plugin trait definitions
//!
//! This module defines the core traits that plugins must implement.
//! All traits are FFI-safe using the #[sabi_trait] attribute.

use crate::{
    data::FfiDataFormat,
    sabi_trait,
    RBoxError, RHashMap, RResult, RStr, RString,
};

/// FFI-safe Data Source trait
///
/// Plugins implement this trait to provide data sources.
/// The trait is FFI-safe and can be used across dynamic library boundaries.
#[sabi_trait]
pub trait FfiDataSource: Send + Sync {
    /// Get the name of this data source
    fn name(&self) -> RStr<'_>;

    /// Read data from the source
    ///
    /// # Arguments
    /// * `config` - Configuration parameters as key-value pairs
    ///
    /// # Returns
    /// * `RResult<FfiDataFormat, RBoxError>` - The data or an error
    fn read(&self, config: RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError>;

    /// Validate the configuration
    ///
    /// # Arguments
    /// * `config` - Configuration parameters to validate
    ///
    /// # Returns
    /// * `RResult<(), RBoxError>` - Success or validation error
    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError>;
}

/// FFI-safe Transform trait
///
/// Plugins implement this trait to provide data transformations.
#[sabi_trait]
pub trait FfiTransform: Send + Sync {
    /// Get the name of this transform
    fn name(&self) -> RStr<'_>;

    /// Apply the transform to data
    ///
    /// # Arguments
    /// * `data` - Input data to transform
    /// * `config` - Optional configuration parameters
    ///
    /// # Returns
    /// * `RResult<FfiDataFormat, RBoxError>` - Transformed data or error
    fn apply(
        &self,
        data: FfiDataFormat,
        config: RHashMap<RString, RString>,
    ) -> RResult<FfiDataFormat, RBoxError>;

    /// Validate the configuration
    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError>;
}

/// FFI-safe Sink trait
///
/// Plugins implement this trait to provide data sinks.
#[sabi_trait]
pub trait FfiSink: Send + Sync {
    /// Get the name of this sink
    fn name(&self) -> RStr<'_>;

    /// Write data to the sink
    ///
    /// # Arguments
    /// * `data` - Data to write
    /// * `config` - Configuration parameters
    ///
    /// # Returns
    /// * `RResult<(), RBoxError>` - Success or error
    fn write(
        &self,
        data: FfiDataFormat,
        config: RHashMap<RString, RString>,
    ) -> RResult<(), RBoxError>;

    /// Validate the configuration
    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RBox, ROk};

    // Test implementation of FfiDataSource
    struct TestSource;

    impl FfiDataSource for TestSource {
        fn name(&self) -> RStr<'_> {
            "test_source".into()
        }

        fn read(&self, _config: RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
            ROk(FfiDataFormat::from_raw(vec![1, 2, 3]))
        }

        fn validate_config(
            &self,
            _config: RHashMap<RString, RString>,
        ) -> RResult<(), RBoxError> {
            ROk(())
        }
    }

    // Test implementation of FfiTransform
    struct TestTransform;

    impl FfiTransform for TestTransform {
        fn name(&self) -> RStr<'_> {
            "test_transform".into()
        }

        fn apply(
            &self,
            data: FfiDataFormat,
            _config: RHashMap<RString, RString>,
        ) -> RResult<FfiDataFormat, RBoxError> {
            // Just pass through the data
            ROk(data)
        }

        fn validate_config(
            &self,
            _config: RHashMap<RString, RString>,
        ) -> RResult<(), RBoxError> {
            ROk(())
        }
    }

    // Test implementation of FfiSink
    struct TestSink;

    impl FfiSink for TestSink {
        fn name(&self) -> RStr<'_> {
            "test_sink".into()
        }

        fn write(
            &self,
            _data: FfiDataFormat,
            _config: RHashMap<RString, RString>,
        ) -> RResult<(), RBoxError> {
            ROk(())
        }

        fn validate_config(
            &self,
            _config: RHashMap<RString, RString>,
        ) -> RResult<(), RBoxError> {
            ROk(())
        }
    }

    #[test]
    fn test_data_source_trait() {
        let source = TestSource;
        assert_eq!(source.name(), "test_source");

        let config = RHashMap::new();
        let result = source.read(config.clone());
        assert!(result.is_ok());

        let validate_result = source.validate_config(config);
        assert!(validate_result.is_ok());
    }

    #[test]
    fn test_transform_trait() {
        let transform = TestTransform;
        assert_eq!(transform.name(), "test_transform");

        let data = FfiDataFormat::from_raw(vec![1, 2, 3]);
        let config = RHashMap::new();

        let result = transform.apply(data, config.clone());
        assert!(result.is_ok());

        let validate_result = transform.validate_config(config);
        assert!(validate_result.is_ok());
    }

    #[test]
    fn test_sink_trait() {
        let sink = TestSink;
        assert_eq!(sink.name(), "test_sink");

        let data = FfiDataFormat::from_raw(vec![1, 2, 3]);
        let config = RHashMap::new();

        let result = sink.write(data, config.clone());
        assert!(result.is_ok());

        let validate_result = sink.validate_config(config);
        assert!(validate_result.is_ok());
    }

    #[test]
    fn test_trait_objects() {
        use crate::sabi_trait::prelude::*;

        // Test that we can create trait objects
        let source: FfiDataSource_TO<'_, RBox<()>> = FfiDataSource_TO::from_value(TestSource, TD_Opaque);
        assert_eq!(source.name(), "test_source");

        let transform: FfiTransform_TO<'_, RBox<()>> = FfiTransform_TO::from_value(TestTransform, TD_Opaque);
        assert_eq!(transform.name(), "test_transform");

        let sink: FfiSink_TO<'_, RBox<()>> = FfiSink_TO::from_value(TestSink, TD_Opaque);
        assert_eq!(sink.name(), "test_sink");
    }
}
