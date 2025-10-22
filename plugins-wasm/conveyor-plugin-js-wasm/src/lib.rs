//! JavaScript Inline Execution WASM Plugin for Conveyor
//!
//! This plugin allows inline JavaScript code execution for data transformation.
//! Uses Boa engine (pure Rust JavaScript engine) for safe execution.

use boa_engine::{Context, Source};
use std::collections::HashMap;

// Generate WIT bindings
wit_bindgen::generate!({
    world: "plugin",
    path: "../../conveyor-wasm-plugin-api/wit",
});

// Plugin API version
const PLUGIN_API_VERSION: u32 = 1;

// Helper functions
fn get_config_value<'a>(config: &'a [(String, String)], key: &str) -> Option<&'a str> {
    config
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn data_format_from_json<T: serde::Serialize>(records: &T) -> Result<DataFormat, PluginError> {
    match serde_json::to_vec(records) {
        Ok(bytes) => Ok(DataFormat::JsonRecords(bytes)),
        Err(e) => Err(PluginError::SerializationError(format!(
            "Failed to serialize JSON: {}",
            e
        ))),
    }
}

fn data_format_to_bytes(data: &DataFormat) -> &[u8] {
    match data {
        DataFormat::ArrowIpc(b) => b,
        DataFormat::JsonRecords(b) => b,
        DataFormat::Raw(b) => b,
    }
}

/// JavaScript plugin structure
struct JsPlugin;

impl Guest for JsPlugin {
    /// Get plugin metadata
    fn get_metadata() -> PluginMetadata {
        PluginMetadata {
            name: "js-wasm".to_string(),
            version: "0.1.0".to_string(),
            description: "JavaScript inline execution plugin using Boa engine".to_string(),
            api_version: PLUGIN_API_VERSION,
        }
    }

    /// Get list of stages this plugin provides
    fn get_capabilities() -> Vec<StageCapability> {
        vec![StageCapability {
            name: "js.eval".to_string(),
            stage_type: StageType::Transform,
            description: "Execute JavaScript code to transform data".to_string(),
        }]
    }

    /// Execute a stage
    fn execute(stage_name: String, context: ExecutionContext) -> Result<DataFormat, PluginError> {
        // Verify stage name
        if stage_name != "js.eval" {
            return Err(PluginError::ConfigError(format!(
                "Unknown stage: {}. JS plugin only provides 'js.eval' stage",
                stage_name
            )));
        }

        // JS plugin is transform-only
        if context.inputs.is_empty() {
            return Err(PluginError::ConfigError(
                "js.eval requires input data (it's a transform)".to_string(),
            ));
        }

        execute_transform(&context.inputs, &context.config)
    }

    /// Validate configuration for a stage
    fn validate_config(
        stage_name: String,
        config: Vec<(String, String)>,
    ) -> Result<(), PluginError> {
        // Verify stage name
        if stage_name != "js.eval" {
            return Err(PluginError::ConfigError(format!(
                "Unknown stage: {}. JS plugin only provides 'js.eval' stage",
                stage_name
            )));
        }

        // Check required 'script' parameter
        if get_config_value(&config, "script").is_none() {
            return Err(PluginError::ConfigError(
                "Missing required 'script' parameter".to_string(),
            ));
        }

        Ok(())
    }
}

/// Execute JavaScript transform
fn execute_transform(
    inputs: &[(String, DataFormat)],
    config: &[(String, String)],
) -> Result<DataFormat, PluginError> {
    // Get the first input
    let input_data = inputs.first().ok_or_else(|| {
        PluginError::RuntimeError("Transform requires at least one input".to_string())
    })?;

    // Get the JavaScript code
    let script = get_config_value(config, "script").ok_or_else(|| {
        PluginError::ConfigError("Missing required 'script' parameter".to_string())
    })?;

    // Parse input data
    let bytes = data_format_to_bytes(&input_data.1);
    let records: Vec<HashMap<String, serde_json::Value>> = match &input_data.1 {
        DataFormat::JsonRecords(_) => serde_json::from_slice(bytes).map_err(|e| {
            PluginError::SerializationError(format!("Failed to parse JSON input: {}", e))
        })?,
        _ => {
            return Err(PluginError::RuntimeError(
                "js.eval only supports JsonRecords input format".to_string(),
            ));
        }
    };

    // Execute JavaScript for each row
    let mut transformed_records = Vec::new();

    for (idx, row) in records.iter().enumerate() {
        match transform_row(row, script) {
            Ok(transformed_row) => transformed_records.push(transformed_row),
            Err(e) => {
                return Err(PluginError::RuntimeError(format!(
                    "JavaScript error at row {}: {}",
                    idx, e
                )));
            }
        }
    }

    // Return transformed data
    data_format_from_json(&transformed_records)
}

/// Transform a single row using JavaScript
fn transform_row(
    row: &HashMap<String, serde_json::Value>,
    script: &str,
) -> Result<HashMap<String, serde_json::Value>, String> {
    // Create a new JavaScript context
    let mut context = Context::default();

    // Parse and evaluate the script
    context
        .eval(Source::from_bytes(script))
        .map_err(|e| format!("Failed to parse script: {}", e))?;

    // Convert row to JSON string for passing to JS
    let row_json =
        serde_json::to_string(row).map_err(|e| format!("Failed to serialize row: {}", e))?;

    // Call the transform function
    let js_code = format!("JSON.stringify(transform({}));", row_json);
    let result = context
        .eval(Source::from_bytes(&js_code))
        .map_err(|e| format!("Failed to execute transform function: {}", e))?;

    // Convert JS result to string
    let result_str = result
        .to_string(&mut context)
        .map_err(|e| format!("Failed to convert result to string: {}", e))?;

    // Parse result back to HashMap
    let transformed_row: HashMap<String, serde_json::Value> =
        serde_json::from_str(&result_str.to_std_string_escaped())
            .map_err(|e| format!("Failed to parse transformed result: {}", e))?;

    Ok(transformed_row)
}

// Export the plugin implementation
export!(JsPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_row_basic() {
        let mut row = HashMap::new();
        row.insert("name".to_string(), serde_json::json!("Alice"));
        row.insert("age".to_string(), serde_json::json!(30));

        let script = r#"
            function transform(row) {
                return {
                    name: row.name,
                    age: row.age,
                    adult: row.age >= 18
                };
            }
        "#;

        let result = transform_row(&row, script).unwrap();
        assert_eq!(result.get("name").unwrap(), &serde_json::json!("Alice"));
        assert_eq!(result.get("age").unwrap(), &serde_json::json!(30));
        assert_eq!(result.get("adult").unwrap(), &serde_json::json!(true));
    }

    #[test]
    fn test_transform_row_computed_fields() {
        let mut row = HashMap::new();
        row.insert("firstName".to_string(), serde_json::json!("John"));
        row.insert("lastName".to_string(), serde_json::json!("Doe"));
        row.insert("birthYear".to_string(), serde_json::json!(1990));

        let script = r#"
            function transform(row) {
                return {
                    firstName: row.firstName,
                    lastName: row.lastName,
                    fullName: row.firstName + ' ' + row.lastName,
                    age: new Date().getFullYear() - row.birthYear
                };
            }
        "#;

        let result = transform_row(&row, script).unwrap();
        assert_eq!(
            result.get("fullName").unwrap(),
            &serde_json::json!("John Doe")
        );
        assert!(result.contains_key("age"));
    }

    #[test]
    fn test_transform_row_missing_function() {
        let mut row = HashMap::new();
        row.insert("value".to_string(), serde_json::json!(42));

        let script = r#"
            // No transform function defined
            var x = 10;
        "#;

        let result = transform_row(&row, script);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("transform function"));
    }

    #[test]
    fn test_transform_row_syntax_error() {
        let mut row = HashMap::new();
        row.insert("value".to_string(), serde_json::json!(42));

        let script = r#"
            function transform(row) {
                return { invalid syntax here
            }
        "#;

        let result = transform_row(&row, script);
        assert!(result.is_err());
    }
}
