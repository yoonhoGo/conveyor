# Metadata System

Conveyor's metadata system provides rich, structured information about every stage (source, transform, sink) in the pipeline. This enables self-documenting CLIs, guided stage creation, and automatic validation.

## Overview

Every stage in Conveyor has associated metadata that describes:
- What it does (description)
- What parameters it accepts (with types, defaults, validation rules)
- How to use it (examples)
- How to find it (tags, categories)

This metadata is **embedded in the code** as part of each stage implementation, ensuring documentation is always up-to-date and accessible without external files.

## Core Concepts

### StageMetadata

The main metadata structure:

```rust
pub struct StageMetadata {
    pub name: String,                    // Function name (e.g., "csv.read")
    pub category: StageCategory,         // Source, Transform, or Sink
    pub description: String,             // Short one-line description
    pub long_description: Option<String>, // Detailed explanation
    pub parameters: Vec<ConfigParameter>, // All configuration parameters
    pub examples: Vec<ConfigExample>,     // Usage examples
    pub tags: Vec<String>,                // Searchable tags
}
```

### ConfigParameter

Describes a single configuration parameter:

```rust
pub struct ConfigParameter {
    pub name: String,                    // Parameter name
    pub param_type: ParameterType,       // Data type
    pub required: bool,                  // Is it required?
    pub default: Option<String>,         // Default value if optional
    pub description: String,             // What it does
    pub validation: Option<ParameterValidation>, // Constraints
}
```

**Parameter Types:**
- `String` - Text values
- `Integer` - Whole numbers
- `Float` - Decimal numbers
- `Boolean` - true/false
- `Array` - Lists of values
- `Object` - Nested structures

### ParameterValidation

Defines validation rules:

```rust
pub struct ParameterValidation {
    pub min: Option<f64>,                // Minimum value (numbers)
    pub max: Option<f64>,                // Maximum value (numbers)
    pub allowed_values: Option<Vec<String>>, // Enum options
    pub pattern: Option<String>,         // Regex pattern (strings)
    pub min_length: Option<usize>,       // Minimum length (strings/arrays)
    pub max_length: Option<usize>,       // Maximum length (strings/arrays)
}
```

**Common Validation Scenarios:**

1. **Enum (allowed values):**
   ```rust
   ParameterValidation::allowed_values(["csv", "json", "parquet"])
   ```

2. **Numeric range:**
   ```rust
   ParameterValidation::range(0.0, 100.0)
   ```

3. **String pattern:**
   ```rust
   ParameterValidation::pattern(r"^\d{4}-\d{2}-\d{2}$") // Date format
   ```

4. **Length constraints:**
   ```rust
   ParameterValidation {
       min_length: Some(1),
       max_length: Some(255),
       ..Default::default()
   }
   ```

## Implementing Metadata for a Stage

### Step 1: Import Required Types

```rust
use crate::core::metadata::{
    ConfigParameter,
    ParameterType,
    ParameterValidation,
    StageCategory,
    StageMetadata,
};
```

### Step 2: Implement the `metadata()` Method

Every stage must implement `metadata()`:

```rust
#[async_trait]
impl Stage for MyStage {
    fn name(&self) -> &str {
        "my.stage"
    }

    fn metadata(&self) -> StageMetadata {
        // Build metadata here
    }

    // ... other trait methods
}
```

### Step 3: Use the Builder Pattern

```rust
fn metadata(&self) -> StageMetadata {
    StageMetadata::builder("my.stage", StageCategory::Transform)
        .description("Short one-line description")
        .long_description(
            "Detailed explanation of what this stage does. \
            Can span multiple lines and include technical details."
        )
        .parameter(ConfigParameter::required(
            "input_field",
            ParameterType::String,
            "Name of the field to process"
        ))
        .parameter(ConfigParameter::optional(
            "threshold",
            ParameterType::Integer,
            "100",
            "Processing threshold value"
        ).with_validation(ParameterValidation::range(0.0, 1000.0)))
        .example(ConfigExample::new(
            "Basic usage",
            example_config,
            Some("Process user_name field with default threshold")
        ))
        .tag("transform")
        .tag("processing")
        .build()
}
```

## Complete Example

Here's a complete example for a filter stage:

```rust
use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation,
    StageCategory, StageMetadata
};

impl Stage for FilterTransform {
    fn name(&self) -> &str {
        "filter.apply"
    }

    fn metadata(&self) -> StageMetadata {
        // Create example configurations
        let mut example1 = HashMap::new();
        example1.insert("column".to_string(), toml::Value::String("age".to_string()));
        example1.insert("operator".to_string(), toml::Value::String(">=".to_string()));
        example1.insert("value".to_string(), toml::Value::Integer(18));

        let mut example2 = HashMap::new();
        example2.insert("column".to_string(), toml::Value::String("status".to_string()));
        example2.insert("operator".to_string(), toml::Value::String("in".to_string()));
        example2.insert("value".to_string(), toml::Value::Array(vec![
            toml::Value::String("active".to_string()),
            toml::Value::String("pending".to_string()),
        ]));

        StageMetadata::builder("filter.apply", StageCategory::Transform)
            .description("Filter rows based on column values")
            .long_description(
                "Filters DataFrame rows using various comparison operators. \
                Supports numeric comparisons (>, >=, <, <=), equality checks (==, !=), \
                string operations (contains), and set membership (in). \
                Uses Polars' lazy evaluation for optimal performance."
            )
            .parameter(ConfigParameter::required(
                "column",
                ParameterType::String,
                "Name of the column to filter on"
            ))
            .parameter(ConfigParameter::required(
                "value",
                ParameterType::String,
                "Value to compare against"
            ))
            .parameter(ConfigParameter::optional(
                "operator",
                ParameterType::String,
                "==",
                "Comparison operator to use"
            ).with_validation(ParameterValidation::allowed_values([
                "==", "=", "!=", "<>", ">", ">=", "<", "<=", "contains", "in"
            ])))
            .example(ConfigExample::new(
                "Filter adults (age >= 18)",
                example1,
                Some("Keep only rows where age is 18 or greater")
            ))
            .example(ConfigExample::new(
                "Filter by status list",
                example2,
                Some("Keep rows where status is 'active' or 'pending'")
            ))
            .tag("filter")
            .tag("transform")
            .tag("dataframe")
            .build()
    }

    // ... execute, validate_config, etc.
}
```

## Best Practices

### 1. Clear Descriptions

**Good:**
```rust
.description("Filter rows based on column values")
```

**Bad:**
```rust
.description("Filter") // Too vague
```

### 2. Comprehensive Long Descriptions

Include:
- What the stage does
- How it works
- Performance characteristics
- Limitations or caveats

```rust
.long_description(
    "Filters DataFrame rows using various comparison operators. \
    Supports numeric comparisons (>, >=, <, <=), equality checks (==, !=), \
    string operations (contains), and set membership (in). \
    Uses Polars' lazy evaluation for optimal performance."
)
```

### 3. Descriptive Parameter Names

**Good:**
```rust
ConfigParameter::required(
    "delimiter",
    ParameterType::String,
    "Field delimiter character (must be a single character)"
)
```

**Bad:**
```rust
ConfigParameter::required(
    "delim",
    ParameterType::String,
    "Delimiter"
)
```

### 4. Always Provide Validation for Enums

```rust
.parameter(ConfigParameter::optional(
    "format",
    ParameterType::String,
    "csv",
    "Output format"
).with_validation(ParameterValidation::allowed_values([
    "csv", "json", "parquet", "arrow"
])))
```

This enables:
- Automatic validation in CLI
- Auto-completion hints
- Clear error messages

### 5. Include Real Examples

Examples should be realistic and copy-pastable:

```rust
let mut example = HashMap::new();
example.insert("path".to_string(), toml::Value::String("data.csv".to_string()));
example.insert("headers".to_string(), toml::Value::Boolean(true));
example.insert("delimiter".to_string(), toml::Value::String(",".to_string()));

.example(ConfigExample::new(
    "Basic CSV reading",
    example,
    Some("Read a CSV file with headers and comma delimiter")
))
```

### 6. Use Meaningful Tags

Tags improve discoverability:

```rust
.tag("csv")      // File format
.tag("file")     // I/O type
.tag("io")       // Operation category
.tag("source")   // Stage type
```

Choose tags that users might search for.

## Using Metadata in CLI

### List Functions with Descriptions

```bash
$ conveyor list
SOURCES:
  • csv.read                  - Read data from CSV files
  • json.read                 - Read data from JSON files
```

### Get Detailed Information

```bash
$ conveyor info csv.read

======================================================================
Function: csv.read
Category: source
======================================================================

Read data from CSV files

Reads CSV (Comma-Separated Values) files and converts them into a
DataFrame. Supports custom delimiters, header configuration, and
automatic schema inference.

PARAMETERS:
----------------------------------------------------------------------

Required:
  • path (string)
    Path to the CSV file to read

Optional:
  • headers (boolean) [default: true]
    Whether the first row contains column headers
  • delimiter (string) [default: ,]
    Field delimiter character (must be a single character)
    ...
```

### Interactive Stage Building

```bash
$ conveyor build

# Guided prompts with validation based on metadata:
# - Type checking
# - Enum validation
# - Range validation
# - Default values
# - Error messages with retry
```

### Export as JSON

```bash
$ conveyor stage describe csv.read

{
  "name": "csv.read",
  "category": "Source",
  "description": "Read data from CSV files",
  "parameters": [...],
  "examples": [...],
  "tags": ["csv", "file", "io", "source"]
}
```

## Testing Metadata

Always test your metadata implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        let stage = MyStage::new();
        let metadata = stage.metadata();

        // Check basic properties
        assert_eq!(metadata.name, "my.stage");
        assert_eq!(metadata.category, StageCategory::Transform);

        // Check required parameters
        let required = metadata.required_parameters();
        assert!(!required.is_empty());

        // Check specific parameter exists
        let param = metadata.get_parameter("input_field");
        assert!(param.is_some());

        // Check validation
        if let Some(p) = param {
            assert!(p.required);
            assert_eq!(p.param_type, ParameterType::String);
        }
    }
}
```

## Plugin Metadata

Plugins (both FFI and WASM) also provide metadata through `PluginCapability`:

```rust
pub struct PluginCapability {
    pub name: RString,           // Stage name
    pub stage_type: StageType,   // Source/Transform/Sink
    pub description: RString,    // Description
    pub factory_symbol: RString, // Factory function name
}
```

The adapter automatically converts this to `StageMetadata`:

```rust
impl Stage for FfiPluginStageAdapter {
    fn metadata(&self) -> StageMetadata {
        StageMetadata::builder(&self.name, self.category())
            .description(&self.description)
            .tag("plugin")
            .tag("ffi")
            .build()
    }
}
```

## Future Enhancements

Potential future additions to the metadata system:

1. **Parameter Dependencies**: Express when one parameter requires another
2. **Conditional Validation**: Validation rules that depend on other parameters
3. **Rich Types**: Support for more complex parameter types (unions, tuples)
4. **Documentation Links**: URLs to external documentation
5. **Version History**: Track parameter changes across versions
6. **Performance Hints**: Expected memory/CPU usage
7. **Compatibility**: Minimum/maximum versions of dependencies

## Summary

The metadata system makes Conveyor self-documenting and user-friendly:

- ✅ **Built-in documentation**: No external files needed
- ✅ **Interactive guidance**: Helps users build correct configurations
- ✅ **Automatic validation**: Catches errors before execution
- ✅ **Programmatic access**: JSON export for tooling
- ✅ **Always up-to-date**: Documentation lives with code
- ✅ **Consistent UX**: Same experience across all stages

By implementing rich metadata for every stage, we create a CLI that teaches users how to use it effectively.
