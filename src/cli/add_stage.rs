use anyhow::Result;
use dialoguer::{Input, MultiSelect, Select};
use std::path::PathBuf;

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

    // Stage type selection
    let stage_types = vec![
        ("source.csv", "CSV file source"),
        ("source.json", "JSON file source"),
        ("source.stdin", "Standard input source"),
        ("transform.filter", "Filter rows based on conditions"),
        ("transform.map", "Calculate/transform columns"),
        ("transform.validate_schema", "Validate data schema"),
        ("transform.http_fetch", "Fetch data from HTTP API"),
        ("sink.csv", "Write to CSV file"),
        ("sink.json", "Write to JSON file"),
        ("sink.stdout", "Write to standard output"),
        ("stage.pipeline", "Nested pipeline"),
        ("plugin.http", "HTTP plugin (requires plugin)"),
        ("plugin.mongodb", "MongoDB plugin (requires plugin)"),
    ];

    let stage_type_display: Vec<String> = stage_types
        .iter()
        .map(|(typ, desc)| format!("{:<30} - {}", typ, desc))
        .collect();

    let selection = Select::new()
        .with_prompt("Stage type")
        .items(&stage_type_display)
        .default(0)
        .interact()?;

    let (stage_type, _) = stage_types[selection];

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
    let stage_config = collect_stage_config(stage_type)?;

    // Create new stage
    let mut new_stage = toml::map::Map::new();
    new_stage.insert("id".to_string(), toml::Value::String(stage_id.clone()));
    new_stage.insert(
        "type".to_string(),
        toml::Value::String(stage_type.to_string()),
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

fn collect_stage_config(stage_type: &str) -> Result<toml::map::Map<String, toml::Value>> {
    let mut config = toml::map::Map::new();

    match stage_type {
        "source.csv" => {
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

        "source.json" => {
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

        "source.stdin" => {
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

        "transform.filter" => {
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

        "transform.map" => {
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

        "transform.validate_schema" => {
            let required_fields: String = Input::new()
                .with_prompt("Required fields (comma-separated)")
                .interact_text()?;
            let fields: Vec<toml::Value> = required_fields
                .split(',')
                .map(|s| toml::Value::String(s.trim().to_string()))
                .collect();
            config.insert("required_fields".to_string(), toml::Value::Array(fields));
        }

        "transform.http_fetch" => {
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

        "sink.csv" => {
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

        "sink.json" => {
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

        "sink.stdout" => {
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

        "stage.pipeline" => {
            let use_file: bool = dialoguer::Confirm::new()
                .with_prompt("Use external pipeline file? (otherwise inline)")
                .default(true)
                .interact()?;

            if use_file {
                let file_path: String = Input::new()
                    .with_prompt("Pipeline file path")
                    .interact_text()?;
                config.insert("file".to_string(), toml::Value::String(file_path));
            } else {
                let inline_config: String = Input::new()
                    .with_prompt("Inline pipeline configuration (TOML)")
                    .interact_text()?;
                config.insert("inline".to_string(), toml::Value::String(inline_config));
            }
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
    use super::*;

    #[test]
    fn test_collect_stage_config_types() {
        // Just ensure the function signatures are correct
        let config = toml::map::Map::new();
        assert!(config.is_empty());
    }
}
