use anyhow::Result;
use std::path::PathBuf;

use crate::core::registry::ModuleRegistry;

pub fn list_modules(module_type: Option<String>) -> Result<()> {
    let registry = ModuleRegistry::with_defaults();
    let modules = registry.list_all_modules();

    if let Some(filter_type) = module_type {
        if let Some(module_list) = modules.get(&filter_type) {
            println!("\n{} modules:", filter_type.to_uppercase());
            println!("{}", "-".repeat(40));
            for module in module_list {
                println!("  • {}", module);
            }
        } else {
            println!("Unknown module type: {}", filter_type);
            println!("Available types: sources, transforms, sinks");
        }
    } else {
        println!("\nAvailable Modules");
        println!("{}", "=".repeat(40));

        for (category, module_list) in modules {
            println!("\n{}:", category.to_uppercase());
            println!("{}", "-".repeat(40));
            for module in module_list {
                println!("  • {}", module);
            }
        }
    }

    println!();
    Ok(())
}

pub fn generate_sample(output: Option<PathBuf>) -> Result<()> {
    let sample_config = r#"# Conveyor ETL Pipeline Configuration
# This is a sample configuration file demonstrating various features

[pipeline]
name = "sample_pipeline"
version = "1.0.0"
description = "A sample ETL pipeline configuration"

[global]
log_level = "info"  # Options: trace, debug, info, warn, error
max_parallel_tasks = 4
timeout_seconds = 300

# ============================================
# DATA SOURCES
# ============================================

# CSV File Source
[[sources]]
name = "csv_source"
type = "csv"

[sources.config]
path = "data/input.csv"
headers = true
delimiter = ","
# infer_schema_length = 100  # Optional

# JSON File Source
# [[sources]]
# name = "json_source"
# type = "json"
#
# [sources.config]
# path = "data/input.json"
# format = "records"  # Options: records, jsonl, dataframe

# Standard Input Source
# [[sources]]
# name = "stdin_source"
# type = "stdin"
#
# [sources.config]
# format = "json"  # Options: json, jsonl, csv, raw

# ============================================
# TRANSFORMATIONS
# ============================================

# Validate Schema
[[transforms]]
name = "validate"
function = "validate_schema"

[transforms.config]
required_fields = ["id", "name", "value"]
non_nullable = ["id"]
# unique_fields = ["id"]
# field_types = { id = "int", name = "string", value = "float" }

# Filter Data
[[transforms]]
name = "filter"
function = "filter"

[transforms.config]
column = "value"
operator = ">="  # Options: ==, !=, >, >=, <, <=, contains, in
value = 100.0

# Map/Calculate New Column
[[transforms]]
name = "calculate"
function = "map"

[transforms.config]
expression = "value * 1.2"  # Simple arithmetic expressions
output_column = "adjusted_value"

# ============================================
# DATA SINKS
# ============================================

# CSV Output
[[sinks]]
name = "csv_output"
type = "csv"

[sinks.config]
path = "output/result.csv"
headers = true
delimiter = ","

# JSON Output
# [[sinks]]
# name = "json_output"
# type = "json"
#
# [sinks.config]
# path = "output/result.json"
# format = "records"  # Options: records, jsonl, dataframe
# pretty = true

# Standard Output
# [[sinks]]
# name = "stdout_output"
# type = "stdout"
#
# [sinks.config]
# format = "table"  # Options: table, json, jsonl, csv
# limit = 10  # Optional: limit number of rows

# ============================================
# ERROR HANDLING
# ============================================

[error_handling]
strategy = "stop"  # Options: stop, continue, retry
max_retries = 3
retry_delay_seconds = 5

# Dead Letter Queue for failed records
# [error_handling.dead_letter_queue]
# enabled = true
# path = "errors/"
"#;

    if let Some(path) = output {
        std::fs::write(&path, sample_config)?;
        println!("✓ Sample configuration written to: {:?}", path);
    } else {
        println!("{}", sample_config);
    }

    Ok(())
}