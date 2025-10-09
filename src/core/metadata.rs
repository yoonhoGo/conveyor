//! Stage metadata system
//!
//! Provides metadata information for stages including:
//! - Description and documentation
//! - Configuration parameters with types and validation
//! - Usage examples
//! - Tags and categorization

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata for a stage (source, transform, or sink)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageMetadata {
    /// Stage name (e.g., "csv.read", "filter.apply")
    pub name: String,

    /// Stage category
    pub category: StageCategory,

    /// Short description (one line)
    pub description: String,

    /// Detailed description (optional, can be multiple paragraphs)
    pub long_description: Option<String>,

    /// Configuration parameters
    pub parameters: Vec<ConfigParameter>,

    /// Usage examples
    pub examples: Vec<ConfigExample>,

    /// Tags for categorization and search
    pub tags: Vec<String>,
}

impl StageMetadata {
    /// Create a new stage metadata builder
    pub fn builder(name: impl Into<String>, category: StageCategory) -> MetadataBuilder {
        MetadataBuilder {
            name: name.into(),
            category,
            description: String::new(),
            long_description: None,
            parameters: Vec::new(),
            examples: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Get required parameters
    pub fn required_parameters(&self) -> Vec<&ConfigParameter> {
        self.parameters.iter().filter(|p| p.required).collect()
    }

    /// Get optional parameters
    pub fn optional_parameters(&self) -> Vec<&ConfigParameter> {
        self.parameters.iter().filter(|p| !p.required).collect()
    }

    /// Get parameter by name
    pub fn get_parameter(&self, name: &str) -> Option<&ConfigParameter> {
        self.parameters.iter().find(|p| p.name == name)
    }
}

/// Builder for StageMetadata
pub struct MetadataBuilder {
    name: String,
    category: StageCategory,
    description: String,
    long_description: Option<String>,
    parameters: Vec<ConfigParameter>,
    examples: Vec<ConfigExample>,
    tags: Vec<String>,
}

impl MetadataBuilder {
    /// Set short description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set long description
    pub fn long_description(mut self, desc: impl Into<String>) -> Self {
        self.long_description = Some(desc.into());
        self
    }

    /// Add a parameter
    pub fn parameter(mut self, param: ConfigParameter) -> Self {
        self.parameters.push(param);
        self
    }

    /// Add an example
    pub fn example(mut self, example: ConfigExample) -> Self {
        self.examples.push(example);
        self
    }

    /// Add a tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(|t| t.into()));
        self
    }

    /// Build the metadata
    pub fn build(self) -> StageMetadata {
        StageMetadata {
            name: self.name,
            category: self.category,
            description: self.description,
            long_description: self.long_description,
            parameters: self.parameters,
            examples: self.examples,
            tags: self.tags,
        }
    }
}

/// Stage category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageCategory {
    /// Data source (reads data)
    Source,
    /// Data transformer (processes data)
    Transform,
    /// Data sink (writes data)
    Sink,
}

impl StageCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            StageCategory::Source => "source",
            StageCategory::Transform => "transform",
            StageCategory::Sink => "sink",
        }
    }
}

/// Configuration parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigParameter {
    /// Parameter name (as used in TOML config)
    pub name: String,

    /// Parameter type
    pub param_type: ParameterType,

    /// Whether this parameter is required
    pub required: bool,

    /// Default value (if optional)
    pub default: Option<String>,

    /// Parameter description
    pub description: String,

    /// Validation rules
    pub validation: Option<ParameterValidation>,
}

impl ConfigParameter {
    /// Create a new required parameter
    pub fn required(
        name: impl Into<String>,
        param_type: ParameterType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            param_type,
            required: true,
            default: None,
            description: description.into(),
            validation: None,
        }
    }

    /// Create a new optional parameter
    pub fn optional(
        name: impl Into<String>,
        param_type: ParameterType,
        default: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            param_type,
            required: false,
            default: Some(default.into()),
            description: description.into(),
            validation: None,
        }
    }

    /// Add validation rules
    pub fn with_validation(mut self, validation: ParameterValidation) -> Self {
        self.validation = Some(validation);
        self
    }
}

/// Parameter type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

impl ParameterType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParameterType::String => "string",
            ParameterType::Integer => "integer",
            ParameterType::Float => "float",
            ParameterType::Boolean => "boolean",
            ParameterType::Array => "array",
            ParameterType::Object => "object",
        }
    }
}

/// Parameter validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterValidation {
    /// Minimum value (for numbers)
    pub min: Option<f64>,

    /// Maximum value (for numbers)
    pub max: Option<f64>,

    /// Allowed values (enum)
    pub allowed_values: Option<Vec<String>>,

    /// Regular expression pattern (for strings)
    pub pattern: Option<String>,

    /// Minimum length (for strings/arrays)
    pub min_length: Option<usize>,

    /// Maximum length (for strings/arrays)
    pub max_length: Option<usize>,
}

impl ParameterValidation {
    /// Create validation with allowed values
    pub fn allowed_values(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            min: None,
            max: None,
            allowed_values: Some(values.into_iter().map(|v| v.into()).collect()),
            pattern: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create validation with numeric range
    pub fn range(min: f64, max: f64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
            allowed_values: None,
            pattern: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create validation with string pattern
    pub fn pattern(pattern: impl Into<String>) -> Self {
        Self {
            min: None,
            max: None,
            allowed_values: None,
            pattern: Some(pattern.into()),
            min_length: None,
            max_length: None,
        }
    }
}

/// Configuration example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigExample {
    /// Example title/name
    pub title: String,

    /// Example configuration
    pub config: HashMap<String, toml::Value>,

    /// Optional description of what this example does
    pub description: Option<String>,
}

impl ConfigExample {
    /// Create a new example
    pub fn new(
        title: impl Into<String>,
        config: HashMap<String, toml::Value>,
        description: Option<impl Into<String>>,
    ) -> Self {
        Self {
            title: title.into(),
            config,
            description: description.map(|d| d.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_builder() {
        let metadata = StageMetadata::builder("csv.read", StageCategory::Source)
            .description("Read data from CSV files")
            .parameter(ConfigParameter::required(
                "path",
                ParameterType::String,
                "Path to the CSV file",
            ))
            .parameter(ConfigParameter::optional(
                "headers",
                ParameterType::Boolean,
                "true",
                "Whether the CSV has headers",
            ))
            .tag("csv")
            .tag("file")
            .build();

        assert_eq!(metadata.name, "csv.read");
        assert_eq!(metadata.category, StageCategory::Source);
        assert_eq!(metadata.parameters.len(), 2);
        assert_eq!(metadata.tags.len(), 2);
    }

    #[test]
    fn test_required_optional_parameters() {
        let metadata = StageMetadata::builder("test", StageCategory::Transform)
            .description("Test stage")
            .parameter(ConfigParameter::required(
                "required_param",
                ParameterType::String,
                "A required parameter",
            ))
            .parameter(ConfigParameter::optional(
                "optional_param",
                ParameterType::Integer,
                "10",
                "An optional parameter",
            ))
            .build();

        let required = metadata.required_parameters();
        let optional = metadata.optional_parameters();

        assert_eq!(required.len(), 1);
        assert_eq!(optional.len(), 1);
        assert_eq!(required[0].name, "required_param");
        assert_eq!(optional[0].name, "optional_param");
    }

    #[test]
    fn test_parameter_validation() {
        let validation = ParameterValidation::allowed_values(["==", "!=", ">", "<"]);
        assert!(validation.allowed_values.is_some());
        assert_eq!(validation.allowed_values.as_ref().unwrap().len(), 4);

        let range_validation = ParameterValidation::range(0.0, 100.0);
        assert_eq!(range_validation.min, Some(0.0));
        assert_eq!(range_validation.max, Some(100.0));
    }

    #[test]
    fn test_stage_category() {
        assert_eq!(StageCategory::Source.as_str(), "source");
        assert_eq!(StageCategory::Transform.as_str(), "transform");
        assert_eq!(StageCategory::Sink.as_str(), "sink");
    }

    #[test]
    fn test_parameter_type() {
        assert_eq!(ParameterType::String.as_str(), "string");
        assert_eq!(ParameterType::Integer.as_str(), "integer");
        assert_eq!(ParameterType::Boolean.as_str(), "boolean");
    }
}
