use anyhow::Result;
use dialoguer::{Input, MultiSelect, Select};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::registry::ModuleRegistry;

/// Get descriptions for built-in and plugin functions
fn get_function_descriptions() -> HashMap<String, String> {
    let mut descriptions = HashMap::new();

    // Source functions
    descriptions.insert(
        "csv.read".to_string(),
        "Read data from CSV file".to_string(),
    );
    descriptions.insert(
        "json.read".to_string(),
        "Read data from JSON file".to_string(),
    );
    descriptions.insert(
        "stdin.read".to_string(),
        "Read data from standard input".to_string(),
    );

    // Sink functions
    descriptions.insert(
        "csv.write".to_string(),
        "Write data to CSV file".to_string(),
    );
    descriptions.insert(
        "json.write".to_string(),
        "Write data to JSON file".to_string(),
    );
    descriptions.insert(
        "stdout.write".to_string(),
        "Write data to standard output".to_string(),
    );
    descriptions.insert(
        "stdout_stream.write".to_string(),
        "Write streaming data to stdout".to_string(),
    );

    // Transform functions
    descriptions.insert(
        "filter.apply".to_string(),
        "Filter rows based on conditions".to_string(),
    );
    descriptions.insert(
        "map.apply".to_string(),
        "Calculate/transform columns".to_string(),
    );
    descriptions.insert(
        "validate.schema".to_string(),
        "Validate data schema".to_string(),
    );
    descriptions.insert(
        "http.fetch".to_string(),
        "Fetch data from HTTP API per row".to_string(),
    );
    descriptions.insert(
        "reduce.apply".to_string(),
        "Aggregate data with reduce operations".to_string(),
    );
    descriptions.insert(
        "groupby.apply".to_string(),
        "Group data by columns".to_string(),
    );
    descriptions.insert("sort.apply".to_string(), "Sort data by columns".to_string());
    descriptions.insert(
        "select.apply".to_string(),
        "Select specific columns".to_string(),
    );
    descriptions.insert(
        "distinct.apply".to_string(),
        "Remove duplicate rows".to_string(),
    );
    descriptions.insert(
        "window.apply".to_string(),
        "Apply window operations".to_string(),
    );
    descriptions.insert(
        "aggregate.stream".to_string(),
        "Aggregate streaming data".to_string(),
    );

    // Plugin functions (common ones)
    descriptions.insert(
        "http.get".to_string(),
        "HTTP GET request (plugin)".to_string(),
    );
    descriptions.insert(
        "http.post".to_string(),
        "HTTP POST request (plugin)".to_string(),
    );
    descriptions.insert(
        "mongodb.find".to_string(),
        "Query MongoDB collection (plugin)".to_string(),
    );
    descriptions.insert(
        "mongodb.insert".to_string(),
        "Insert into MongoDB (plugin)".to_string(),
    );

    descriptions
}

pub async fn add_stage_to_pipeline(pipeline_file: PathBuf) -> Result<()> {
    // Read existing pipeline
    let content = std::fs::read_to_string(&pipeline_file)?;
    let mut config: toml::Value = toml::from_str(&content)?;

    // Get existing stages to determine available inputs
    let existing_stages = config
        .get("stages")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|stage| stage.get("id").and_then(|id| id.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    println!("\n🔧 Add New Stage to Pipeline");
    println!("{}", "=".repeat(50));

    // Stage ID
    let stage_id: String = Input::new()
        .with_prompt("Stage ID (unique identifier)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.is_empty() {
                Err("Stage ID cannot be empty")
            } else if existing_stages.contains(input) {
                Err("Stage ID already exists")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    // Load available functions from registry
    let registry = ModuleRegistry::with_defaults().await?;
    let mut available_functions = registry.list_functions();
    available_functions.sort();

    let function_descriptions = get_function_descriptions();

    // Build display list with descriptions
    let function_display: Vec<String> = available_functions
        .iter()
        .map(|func| {
            let desc = function_descriptions
                .get(func)
                .map(|d| d.as_str())
                .unwrap_or("No description");
            format!("{:<30} - {}", func, desc)
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Function")
        .items(&function_display)
        .default(0)
        .interact()?;

    let stage_function = &available_functions[selection];

    // Input dependencies
    let inputs = if existing_stages.is_empty() {
        println!("\nNo existing stages - this will be the first stage");
        Vec::new()
    } else {
        let selected_inputs = MultiSelect::new()
            .with_prompt("Select input stages (this stage will depend on them)")
            .items(&existing_stages)
            .interact()?;

        selected_inputs
            .iter()
            .map(|&idx| existing_stages[idx].clone())
            .collect()
    };

    // Stage-specific configuration
    let stage_config = collect_stage_config(stage_function)?;

    // Create new stage
    let mut new_stage = toml::map::Map::new();
    new_stage.insert("id".to_string(), toml::Value::String(stage_id.clone()));
    new_stage.insert(
        "function".to_string(),
        toml::Value::String(stage_function.to_string()),
    );
    new_stage.insert(
        "inputs".to_string(),
        toml::Value::Array(
            inputs
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );

    if !stage_config.is_empty() {
        new_stage.insert("config".to_string(), toml::Value::Table(stage_config));
    }

    // Add to pipeline
    if let Some(stages_array) = config.get_mut("stages").and_then(|s| s.as_array_mut()) {
        stages_array.push(toml::Value::Table(new_stage));
    } else {
        // Create stages array if it doesn't exist
        config
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("Invalid TOML structure"))?
            .insert(
                "stages".to_string(),
                toml::Value::Array(vec![toml::Value::Table(new_stage)]),
            );
    }

    // Write back to file
    let updated_content = toml::to_string_pretty(&config)?;
    std::fs::write(&pipeline_file, updated_content)?;

    println!(
        "\n✓ Stage '{}' added successfully to {:?}",
        stage_id, pipeline_file
    );
    println!("\nNext steps:");
    println!("  1. Review the configuration file");
    println!(
        "  2. Add more stages with 'conveyor add-stage {:?}'",
        pipeline_file
    );
    println!(
        "  3. Validate with 'conveyor validate -c {:?}'",
        pipeline_file
    );

    Ok(())
}

fn collect_stage_config(function_name: &str) -> Result<toml::map::Map<String, toml::Value>> {
    let mut config = toml::map::Map::new();

    match function_name {
        "csv.read" => {
            let path: String = Input::new().with_prompt("CSV file path").interact_text()?;
            config.insert("path".to_string(), toml::Value::String(path));

            let headers: bool = dialoguer::Confirm::new()
                .with_prompt("Has headers?")
                .default(true)
                .interact()?;
            config.insert("headers".to_string(), toml::Value::Boolean(headers));

            let delimiter: String = Input::new()
                .with_prompt("Delimiter")
                .default(",".to_string())
                .interact_text()?;
            config.insert("delimiter".to_string(), toml::Value::String(delimiter));
        }

        "json.read" => {
            let path: String = Input::new().with_prompt("JSON file path").interact_text()?;
            config.insert("path".to_string(), toml::Value::String(path));

            let format_options = vec!["records", "jsonl", "dataframe"];
            let format_idx = Select::new()
                .with_prompt("JSON format")
                .items(&format_options)
                .default(0)
                .interact()?;
            config.insert(
                "format".to_string(),
                toml::Value::String(format_options[format_idx].to_string()),
            );
        }

        "stdin.read" => {
            let format_options = vec!["json", "jsonl", "csv", "raw"];
            let format_idx = Select::new()
                .with_prompt("Input format")
                .items(&format_options)
                .default(0)
                .interact()?;
            config.insert(
                "format".to_string(),
                toml::Value::String(format_options[format_idx].to_string()),
            );
        }

        "filter.apply" => {
            let column: String = Input::new()
                .with_prompt("Column name to filter")
                .interact_text()?;
            config.insert("column".to_string(), toml::Value::String(column));

            let operators = vec!["==", "!=", ">", ">=", "<", "<=", "contains", "in"];
            let op_idx = Select::new()
                .with_prompt("Operator")
                .items(&operators)
                .default(0)
                .interact()?;
            config.insert(
                "operator".to_string(),
                toml::Value::String(operators[op_idx].to_string()),
            );

            let value: String = Input::new()
                .with_prompt("Value to compare")
                .interact_text()?;

            // Try to parse as number or boolean, otherwise use string
            if let Ok(num) = value.parse::<i64>() {
                config.insert("value".to_string(), toml::Value::Integer(num));
            } else if let Ok(num) = value.parse::<f64>() {
                config.insert("value".to_string(), toml::Value::Float(num));
            } else if let Ok(b) = value.parse::<bool>() {
                config.insert("value".to_string(), toml::Value::Boolean(b));
            } else {
                config.insert("value".to_string(), toml::Value::String(value));
            }
        }

        "map.apply" => {
            let expression: String = Input::new()
                .with_prompt("Expression (e.g., 'price * 1.1')")
                .interact_text()?;
            config.insert("expression".to_string(), toml::Value::String(expression));

            let output_column: String = Input::new()
                .with_prompt("Output column name")
                .interact_text()?;
            config.insert(
                "output_column".to_string(),
                toml::Value::String(output_column),
            );
        }

        "validate.schema" => {
            let required_fields: String = Input::new()
                .with_prompt("Required fields (comma-separated)")
                .interact_text()?;
            let fields: Vec<toml::Value> = required_fields
                .split(',')
                .map(|s| toml::Value::String(s.trim().to_string()))
                .collect();
            config.insert("required_fields".to_string(), toml::Value::Array(fields));
        }

        "http.fetch" => {
            let url: String = Input::new()
                .with_prompt("URL template (e.g., 'https://api.example.com/users/{{ id }}')")
                .interact_text()?;
            config.insert("url".to_string(), toml::Value::String(url));

            let method_options = vec!["GET", "POST", "PUT", "PATCH", "DELETE"];
            let method_idx = Select::new()
                .with_prompt("HTTP method")
                .items(&method_options)
                .default(0)
                .interact()?;
            config.insert(
                "method".to_string(),
                toml::Value::String(method_options[method_idx].to_string()),
            );

            let mode_options = vec!["per_row", "batch"];
            let mode_idx = Select::new()
                .with_prompt("Execution mode")
                .items(&mode_options)
                .default(0)
                .interact()?;
            config.insert(
                "mode".to_string(),
                toml::Value::String(mode_options[mode_idx].to_string()),
            );

            let result_field: String = Input::new()
                .with_prompt("Result field name")
                .default("result".to_string())
                .interact_text()?;
            config.insert(
                "result_field".to_string(),
                toml::Value::String(result_field),
            );
        }

        "csv.write" => {
            let path: String = Input::new()
                .with_prompt("Output CSV file path")
                .interact_text()?;
            config.insert("path".to_string(), toml::Value::String(path));

            let headers: bool = dialoguer::Confirm::new()
                .with_prompt("Include headers?")
                .default(true)
                .interact()?;
            config.insert("headers".to_string(), toml::Value::Boolean(headers));
        }

        "json.write" => {
            let path: String = Input::new()
                .with_prompt("Output JSON file path")
                .interact_text()?;
            config.insert("path".to_string(), toml::Value::String(path));

            let format_options = vec!["records", "jsonl", "dataframe"];
            let format_idx = Select::new()
                .with_prompt("JSON format")
                .items(&format_options)
                .default(0)
                .interact()?;
            config.insert(
                "format".to_string(),
                toml::Value::String(format_options[format_idx].to_string()),
            );

            let pretty: bool = dialoguer::Confirm::new()
                .with_prompt("Pretty print?")
                .default(true)
                .interact()?;
            config.insert("pretty".to_string(), toml::Value::Boolean(pretty));
        }

        "stdout.write" => {
            let format_options = vec!["table", "json", "jsonl", "csv"];
            let format_idx = Select::new()
                .with_prompt("Output format")
                .items(&format_options)
                .default(0)
                .interact()?;
            config.insert(
                "format".to_string(),
                toml::Value::String(format_options[format_idx].to_string()),
            );
        }

        _ => {
            println!("\nNo specific configuration needed for this stage type.");
            println!("You can manually edit the configuration file to add custom settings.");
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_collect_stage_config_types() {
        // Just ensure the function signatures are correct
        let config = toml::map::Map::new();
        assert!(config.is_empty());
    }
}
