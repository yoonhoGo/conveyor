//! FFI-safe metadata types for plugin stages
//!
//! Provides metadata information for stages including:
//! - Description and documentation
//! - Configuration parameters with types and validation
//! - Usage examples

use crate::{RString, RVec, StableAbi};

/// FFI-safe stage metadata
#[repr(C)]
#[derive(StableAbi, Debug, Clone)]
pub struct FfiStageMetadata {
    /// Stage name (e.g., "mongodb-find", "http")
    pub name: RString,

    /// Short description (one line)
    pub description: RString,

    /// Detailed description (optional)
    pub long_description: RString,

    /// Configuration parameters
    pub parameters: RVec<FfiConfigParameter>,

    /// Tags for categorization
    pub tags: RVec<RString>,
}

impl FfiStageMetadata {
    /// Create a new metadata instance
    pub fn new(
        name: impl Into<RString>,
        description: impl Into<RString>,
        long_description: impl Into<RString>,
        parameters: Vec<FfiConfigParameter>,
        tags: Vec<impl Into<RString>>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            long_description: long_description.into(),
            parameters: parameters.into(),
            tags: tags
                .into_iter()
                .map(|t| t.into())
                .collect::<Vec<_>>()
                .into(),
        }
    }

    /// Create a simple metadata with just name and description
    pub fn simple(name: impl Into<RString>, description: impl Into<RString>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            long_description: RString::new(),
            parameters: RVec::new(),
            tags: RVec::new(),
        }
    }
}

/// FFI-safe parameter type
#[repr(C)]
#[derive(StableAbi, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiParameterType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

impl FfiParameterType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FfiParameterType::String => "string",
            FfiParameterType::Integer => "integer",
            FfiParameterType::Float => "float",
            FfiParameterType::Boolean => "boolean",
            FfiParameterType::Array => "array",
            FfiParameterType::Object => "object",
        }
    }
}

/// FFI-safe configuration parameter
#[repr(C)]
#[derive(StableAbi, Debug, Clone)]
pub struct FfiConfigParameter {
    /// Parameter name (as used in TOML config)
    pub name: RString,

    /// Parameter type
    pub param_type: FfiParameterType,

    /// Whether this parameter is required
    pub required: bool,

    /// Default value (if optional)
    pub default_value: RString,

    /// Parameter description
    pub description: RString,

    /// Allowed values (for enum-like parameters)
    pub allowed_values: RVec<RString>,
}

impl FfiConfigParameter {
    /// Create a new required parameter
    pub fn required(
        name: impl Into<RString>,
        param_type: FfiParameterType,
        description: impl Into<RString>,
    ) -> Self {
        Self {
            name: name.into(),
            param_type,
            required: true,
            default_value: RString::new(),
            description: description.into(),
            allowed_values: RVec::new(),
        }
    }

    /// Create a new optional parameter
    pub fn optional(
        name: impl Into<RString>,
        param_type: FfiParameterType,
        default: impl Into<RString>,
        description: impl Into<RString>,
    ) -> Self {
        Self {
            name: name.into(),
            param_type,
            required: false,
            default_value: default.into(),
            description: description.into(),
            allowed_values: RVec::new(),
        }
    }

    /// Add allowed values (enum-like validation)
    pub fn with_allowed_values(
        mut self,
        values: impl IntoIterator<Item = impl Into<RString>>,
    ) -> Self {
        self.allowed_values = values
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<_>>()
            .into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_metadata() {
        let metadata = FfiStageMetadata::simple("test-stage", "A test stage");
        assert_eq!(metadata.name.as_str(), "test-stage");
        assert_eq!(metadata.description.as_str(), "A test stage");
        assert!(metadata.parameters.is_empty());
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn test_required_parameter() {
        let param =
            FfiConfigParameter::required("url", FfiParameterType::String, "MongoDB connection URL");
        assert_eq!(param.name.as_str(), "url");
        assert_eq!(param.param_type, FfiParameterType::String);
        assert!(param.required);
        assert!(param.default_value.is_empty());
    }

    #[test]
    fn test_optional_parameter() {
        let param = FfiConfigParameter::optional(
            "timeout",
            FfiParameterType::Integer,
            "30",
            "Request timeout in seconds",
        );
        assert_eq!(param.name.as_str(), "timeout");
        assert!(!param.required);
        assert_eq!(param.default_value.as_str(), "30");
    }

    #[test]
    fn test_parameter_with_allowed_values() {
        let param =
            FfiConfigParameter::optional("method", FfiParameterType::String, "GET", "HTTP method")
                .with_allowed_values(["GET", "POST", "PUT", "DELETE"]);

        assert_eq!(param.allowed_values.len(), 4);
        assert_eq!(param.allowed_values[0].as_str(), "GET");
    }

    #[test]
    fn test_parameter_type_as_str() {
        assert_eq!(FfiParameterType::String.as_str(), "string");
        assert_eq!(FfiParameterType::Integer.as_str(), "integer");
        assert_eq!(FfiParameterType::Float.as_str(), "float");
        assert_eq!(FfiParameterType::Boolean.as_str(), "boolean");
    }
}
