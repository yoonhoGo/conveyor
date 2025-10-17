use anyhow::Result;

use crate::core::registry::ModuleRegistry;

pub mod add_stage;
pub mod edit;
pub mod interactive_builder;
pub mod plugin;
pub mod scaffold;

pub use add_stage::add_stage_to_pipeline;
pub use edit::edit_pipeline_interactive;
pub use interactive_builder::build_stage_interactive;
pub use scaffold::scaffold_pipeline;

pub async fn show_function_help(function_name: &str) -> Result<()> {
    let registry = ModuleRegistry::with_defaults().await?;

    let stage = registry.get_function(function_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Function '{}' not found. Use 'conveyor list' to see available functions.",
            function_name
        )
    })?;

    let metadata = stage.metadata();

    // Header
    println!("\n{}", "=".repeat(70));
    println!("Function: {}", metadata.name);
    println!("Category: {}", metadata.category.as_str());
    println!("{}", "=".repeat(70));

    // Description
    println!("\n{}", metadata.description);

    if let Some(long_desc) = &metadata.long_description {
        println!("\n{}", long_desc);
    }

    // Parameters
    println!("\n{}", "Parameters:".to_uppercase());
    println!("{}", "-".repeat(70));

    let required = metadata.required_parameters();
    let optional = metadata.optional_parameters();

    if !required.is_empty() {
        println!("\nRequired:");
        for param in required {
            println!("  • {} ({})", param.name, param.param_type.as_str());
            println!("    {}", param.description);
            if let Some(validation) = &param.validation {
                if let Some(allowed) = &validation.allowed_values {
                    println!("    Allowed values: {}", allowed.join(", "));
                }
            }
        }
    }

    if !optional.is_empty() {
        println!("\nOptional:");
        for param in optional {
            let default_str = param.default.as_deref().unwrap_or("none");
            println!(
                "  • {} ({}) [default: {}]",
                param.name,
                param.param_type.as_str(),
                default_str
            );
            println!("    {}", param.description);
            if let Some(validation) = &param.validation {
                if let Some(allowed) = &validation.allowed_values {
                    println!("    Allowed values: {}", allowed.join(", "));
                }
            }
        }
    }

    // Examples
    if !metadata.examples.is_empty() {
        println!("\n{}", "Examples:".to_uppercase());
        println!("{}", "-".repeat(70));

        for (i, example) in metadata.examples.iter().enumerate() {
            println!("\n{}. {}", i + 1, example.title);
            if let Some(desc) = &example.description {
                println!("   {}", desc);
            }
            println!("\n   Config:");
            for (key, value) in &example.config {
                println!("     {} = {}", key, format_toml_value(value));
            }
        }
    }

    // Tags
    if !metadata.tags.is_empty() {
        println!("\n{}", "Tags:".to_uppercase());
        println!("{}", "-".repeat(70));
        println!("  {}", metadata.tags.join(", "));
    }

    println!();
    Ok(())
}

pub async fn describe_function_json(function_name: &str) -> Result<()> {
    let registry = ModuleRegistry::with_defaults().await?;

    let stage = registry.get_function(function_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Function '{}' not found. Use 'conveyor list' to see available functions.",
            function_name
        )
    })?;

    let metadata = stage.metadata();
    let json = serde_json::to_string_pretty(&metadata)?;
    println!("{}", json);

    Ok(())
}

fn format_toml_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => format!("\"{}\"", s),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_toml_value).collect();
            format!("[{}]", items.join(", "))
        }
        toml::Value::Table(table) => {
            let items: Vec<String> = table
                .iter()
                .map(|(k, v)| format!("{} = {}", k, format_toml_value(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
        _ => value.to_string(),
    }
}

pub async fn list_modules(module_type: Option<String>) -> Result<()> {
    let registry = ModuleRegistry::with_defaults().await?;
    let all_functions = registry.list_functions();

    // Collect function metadata
    let mut sources = Vec::new();
    let mut transforms = Vec::new();
    let mut sinks = Vec::new();

    for func_name in &all_functions {
        if let Some(stage) = registry.get_function(func_name) {
            let metadata = stage.metadata();
            let info = (func_name.clone(), metadata.description.clone());

            match metadata.category {
                crate::core::metadata::StageCategory::Source => sources.push(info),
                crate::core::metadata::StageCategory::Transform => transforms.push(info),
                crate::core::metadata::StageCategory::Sink => sinks.push(info),
            }
        }
    }

    if let Some(filter_type) = module_type {
        let module_list = match filter_type.as_str() {
            "sources" => &sources,
            "transforms" => &transforms,
            "sinks" => &sinks,
            _ => {
                println!("Unknown module type: {}", filter_type);
                println!("Available types: sources, transforms, sinks");
                return Ok(());
            }
        };

        println!("\n{} modules:", filter_type.to_uppercase());
        println!("{}", "-".repeat(70));
        for (name, desc) in module_list {
            println!("  • {:25} - {}", name, desc);
        }
    } else {
        println!("\nAvailable Modules");
        println!("{}", "=".repeat(70));

        println!("\nSOURCES:");
        println!("{}", "-".repeat(70));
        for (name, desc) in &sources {
            println!("  • {:25} - {}", name, desc);
        }

        println!("\nTRANSFORMS:");
        println!("{}", "-".repeat(70));
        for (name, desc) in &transforms {
            println!("  • {:25} - {}", name, desc);
        }

        println!("\nSINKS:");
        println!("{}", "-".repeat(70));
        for (name, desc) in &sinks {
            println!("  • {:25} - {}", name, desc);
        }

        println!(
            "\nUse 'conveyor info <function>' for detailed information about a specific function"
        );
    }

    println!();
    Ok(())
}
