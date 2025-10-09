//! Interactive stage builder
//!
//! Provides an interpreter-style interface for creating pipeline stages
//! with guided prompts, validation, and examples.

use anyhow::Result;
use std::collections::HashMap;
use std::io::{self, Write};

use crate::core::metadata::{ConfigParameter, ParameterType, StageMetadata};
use crate::core::registry::ModuleRegistry;

/// Interactive stage builder
pub struct InteractiveBuilder {
    registry: ModuleRegistry,
}

impl InteractiveBuilder {
    /// Create a new interactive builder
    pub async fn new() -> Result<Self> {
        let registry = ModuleRegistry::with_defaults().await?;
        Ok(Self { registry })
    }

    /// Start the interactive stage building process
    pub async fn build_stage(&self) -> Result<HashMap<String, toml::Value>> {
        println!("\n{}", "=".repeat(70));
        println!("Interactive Stage Builder");
        println!("{}", "=".repeat(70));

        // Step 1: Select function
        let function_name = self.select_function()?;

        let stage = self.registry.get_function(&function_name).ok_or_else(|| {
            anyhow::anyhow!("Function '{}' not found", function_name)
        })?;

        let metadata = stage.metadata();

        // Step 2: Show function info
        self.show_function_summary(&metadata);

        // Step 3: Collect parameters
        let mut config = HashMap::new();
        config.insert("function".to_string(), toml::Value::String(function_name.clone()));

        // Ask for stage ID
        let stage_id = self.prompt_input("Stage ID (unique identifier for this stage)", None)?;
        config.insert("id".to_string(), toml::Value::String(stage_id));

        // Ask for inputs (comma-separated list of stage IDs)
        let inputs_help = "Input stage IDs (comma-separated, or empty for sources)";
        let inputs_str = self.prompt_input(inputs_help, Some(""))?;
        let inputs: Vec<toml::Value> = if inputs_str.is_empty() {
            Vec::new()
        } else {
            inputs_str
                .split(',')
                .map(|s| toml::Value::String(s.trim().to_string()))
                .collect()
        };
        config.insert("inputs".to_string(), toml::Value::Array(inputs));

        // Collect stage-specific config
        let stage_config = self.collect_parameters(&metadata)?;
        config.insert("config".to_string(), toml::Value::Table(stage_config));

        Ok(config)
    }

    /// Select a function from the registry
    fn select_function(&self) -> Result<String> {
        let functions = self.registry.list_functions();

        println!("\nAvailable functions:");
        println!("{}", "-".repeat(70));

        // Group by category
        let mut sources = Vec::new();
        let mut transforms = Vec::new();
        let mut sinks = Vec::new();

        for func_name in &functions {
            if let Some(stage) = self.registry.get_function(func_name) {
                let metadata = stage.metadata();
                match metadata.category {
                    crate::core::metadata::StageCategory::Source => sources.push(func_name),
                    crate::core::metadata::StageCategory::Transform => transforms.push(func_name),
                    crate::core::metadata::StageCategory::Sink => sinks.push(func_name),
                }
            }
        }

        if !sources.is_empty() {
            println!("\nSources:");
            for func in &sources {
                println!("  • {}", func);
            }
        }

        if !transforms.is_empty() {
            println!("\nTransforms:");
            for func in &transforms {
                println!("  • {}", func);
            }
        }

        if !sinks.is_empty() {
            println!("\nSinks:");
            for func in &sinks {
                println!("  • {}", func);
            }
        }

        println!();
        let function_name = self.prompt_input("Enter function name", None)?;

        if !functions.contains(&function_name) {
            anyhow::bail!("Function '{}' not found. Use 'conveyor list' to see available functions.", function_name);
        }

        Ok(function_name)
    }

    /// Show a summary of the selected function
    fn show_function_summary(&self, metadata: &StageMetadata) {
        println!("\n{}", "=".repeat(70));
        println!("Function: {} ({})", metadata.name, metadata.category.as_str());
        println!("{}", "=".repeat(70));
        println!("\n{}", metadata.description);

        if let Some(long_desc) = &metadata.long_description {
            println!("\n{}", long_desc);
        }

        println!("\n{} required parameter(s), {} optional parameter(s)",
            metadata.required_parameters().len(),
            metadata.optional_parameters().len()
        );
    }

    /// Collect all parameters for a stage
    fn collect_parameters(&self, metadata: &StageMetadata) -> Result<toml::map::Map<String, toml::Value>> {
        let mut config = toml::map::Map::new();

        println!("\n{}", "Configuring stage parameters:".to_uppercase());
        println!("{}", "-".repeat(70));

        // Required parameters
        let required = metadata.required_parameters();
        if !required.is_empty() {
            println!("\nRequired parameters:");
            for param in required {
                let value = self.prompt_parameter(param, true)?;
                config.insert(param.name.clone(), value);
            }
        }

        // Optional parameters
        let optional = metadata.optional_parameters();
        if !optional.is_empty() {
            println!("\nOptional parameters (press Enter to use default):");
            for param in optional {
                let value = self.prompt_parameter(param, false)?;
                if !value.is_str() || !value.as_str().unwrap().is_empty() {
                    config.insert(param.name.clone(), value);
                }
            }
        }

        Ok(config)
    }

    /// Prompt for a single parameter
    fn prompt_parameter(&self, param: &ConfigParameter, required: bool) -> Result<toml::Value> {
        loop {
            // Build prompt message
            let mut prompt = format!("\n{} ({})", param.name, param.param_type.as_str());
            if !required {
                if let Some(default) = &param.default {
                    prompt.push_str(&format!(" [default: {}]", default));
                }
            }
            prompt.push_str(&format!("\n  {}", param.description));

            // Show validation info
            if let Some(validation) = &param.validation {
                if let Some(allowed) = &validation.allowed_values {
                    prompt.push_str(&format!("\n  Allowed: {}", allowed.join(", ")));
                }
                if let Some(min) = validation.min {
                    prompt.push_str(&format!("\n  Min: {}", min));
                }
                if let Some(max) = validation.max {
                    prompt.push_str(&format!("\n  Max: {}", max));
                }
            }

            // Get input
            let default_val = if required { None } else { param.default.as_deref() };
            let input = self.prompt_input(&prompt, default_val)?;

            // Skip if empty and optional
            if !required && input.is_empty() {
                return Ok(toml::Value::String(String::new()));
            }

            // Validate and convert
            match self.parse_and_validate(&input, param) {
                Ok(value) => return Ok(value),
                Err(e) => {
                    println!("  ❌ Invalid input: {}", e);
                    println!("  Please try again.");
                    continue;
                }
            }
        }
    }

    /// Parse and validate parameter input
    fn parse_and_validate(&self, input: &str, param: &ConfigParameter) -> Result<toml::Value> {
        // Parse based on type
        let value = match param.param_type {
            ParameterType::String => toml::Value::String(input.to_string()),
            ParameterType::Integer => {
                let i = input.parse::<i64>()
                    .map_err(|_| anyhow::anyhow!("Not a valid integer"))?;
                toml::Value::Integer(i)
            }
            ParameterType::Float => {
                let f = input.parse::<f64>()
                    .map_err(|_| anyhow::anyhow!("Not a valid float"))?;
                toml::Value::Float(f)
            }
            ParameterType::Boolean => {
                let b = input.to_lowercase();
                if b == "true" || b == "yes" || b == "1" {
                    toml::Value::Boolean(true)
                } else if b == "false" || b == "no" || b == "0" {
                    toml::Value::Boolean(false)
                } else {
                    anyhow::bail!("Not a valid boolean (use: true/false, yes/no, 1/0)")
                }
            }
            ParameterType::Array => {
                // Parse comma-separated values
                let items: Vec<toml::Value> = input
                    .split(',')
                    .map(|s| toml::Value::String(s.trim().to_string()))
                    .collect();
                toml::Value::Array(items)
            }
            ParameterType::Object => {
                // For now, treat as string (user can edit TOML manually later)
                toml::Value::String(input.to_string())
            }
        };

        // Validate
        if let Some(validation) = &param.validation {
            // Check allowed values
            if let Some(allowed) = &validation.allowed_values {
                if let toml::Value::String(s) = &value {
                    if !allowed.contains(s) {
                        anyhow::bail!("Value must be one of: {}", allowed.join(", "));
                    }
                }
            }

            // Check numeric range
            if let Some(min) = validation.min {
                match &value {
                    toml::Value::Integer(i) => {
                        if (*i as f64) < min {
                            anyhow::bail!("Value must be >= {}", min);
                        }
                    }
                    toml::Value::Float(f) => {
                        if *f < min {
                            anyhow::bail!("Value must be >= {}", min);
                        }
                    }
                    _ => {}
                }
            }

            if let Some(max) = validation.max {
                match &value {
                    toml::Value::Integer(i) => {
                        if (*i as f64) > max {
                            anyhow::bail!("Value must be <= {}", max);
                        }
                    }
                    toml::Value::Float(f) => {
                        if *f > max {
                            anyhow::bail!("Value must be <= {}", max);
                        }
                    }
                    _ => {}
                }
            }

            // Check string length
            if let Some(min_len) = validation.min_length {
                if let toml::Value::String(s) = &value {
                    if s.len() < min_len {
                        anyhow::bail!("Length must be >= {} characters", min_len);
                    }
                }
            }

            if let Some(max_len) = validation.max_length {
                if let toml::Value::String(s) = &value {
                    if s.len() > max_len {
                        anyhow::bail!("Length must be <= {} characters", max_len);
                    }
                }
            }
        }

        Ok(value)
    }

    /// Prompt for input with optional default
    fn prompt_input(&self, message: &str, default: Option<&str>) -> Result<String> {
        print!("\n{}: ", message);
        if let Some(def) = default {
            print!("[{}] ", def);
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            if let Some(def) = default {
                return Ok(def.to_string());
            }
        }

        Ok(input.to_string())
    }
}

/// Build a stage interactively and return TOML representation
pub async fn build_stage_interactive() -> Result<String> {
    let builder = InteractiveBuilder::new().await?;
    let stage_config = builder.build_stage().await?;

    // Convert to TOML
    let toml_str = toml::to_string_pretty(&stage_config)?;

    println!("\n{}", "=".repeat(70));
    println!("Generated Stage Configuration:");
    println!("{}", "=".repeat(70));
    println!("\n{}", toml_str);

    Ok(toml_str)
}
