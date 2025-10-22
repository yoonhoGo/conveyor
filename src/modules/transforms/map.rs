use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::metadata::{ConfigParameter, ParameterType, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct MapTransform;

#[async_trait]
impl Stage for MapTransform {
    fn name(&self) -> &str {
        "map"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert(
            "expression".to_string(),
            toml::Value::String("price * 1.1".to_string()),
        );
        example1.insert(
            "output_column".to_string(),
            toml::Value::String("price_with_tax".to_string()),
        );

        let mut example2 = HashMap::new();
        example2.insert(
            "expression".to_string(),
            toml::Value::String("quantity * price".to_string()),
        );
        example2.insert(
            "output_column".to_string(),
            toml::Value::String("total".to_string()),
        );

        let mut example3 = HashMap::new();
        example3.insert(
            "expression".to_string(),
            toml::Value::String("false".to_string()),
        );
        example3.insert(
            "output_column".to_string(),
            toml::Value::String("active".to_string()),
        );

        let mut example4 = HashMap::new();
        example4.insert(
            "expression".to_string(),
            toml::Value::String("\"pending\"".to_string()),
        );
        example4.insert(
            "output_column".to_string(),
            toml::Value::String("status".to_string()),
        );

        StageMetadata::builder("map", StageCategory::Transform)
            .description("Apply mathematical expressions to create new columns")
            .long_description(
                "Creates new columns by applying mathematical expressions to existing columns. \
                Supports basic arithmetic operations: +, -, *, /. \
                Can operate on single columns with constants or between two columns. \
                Also supports boolean constants (true/false), numeric constants, and string constants (quoted). \
                Useful for calculated fields and data transformations.",
            )
            .parameter(ConfigParameter::required(
                "expression",
                ParameterType::String,
                "Mathematical expression (e.g., 'column * 2', 'col1 + col2') or constant (e.g., 'true', 'false', '42', '\"text\"')",
            ))
            .parameter(ConfigParameter::required(
                "output_column",
                ParameterType::String,
                "Name of the new column to create",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Add tax calculation",
                example1,
                Some("Create price_with_tax column by multiplying price by 1.1"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Calculate total",
                example2,
                Some("Multiply quantity by price to get total"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Add boolean field",
                example3,
                Some("Add active column with false value for all rows"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Add string field",
                example4,
                Some("Add status column with 'pending' value for all rows"),
            ))
            .tag("map")
            .tag("expression")
            .tag("calculation")
            .tag("transform")
            .build()
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Map transform requires input data"))?;

        let expression = config
            .get("expression")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Map requires 'expression' configuration"))?;

        let output_column = config
            .get("output_column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Map requires 'output_column' configuration"))?;

        let mut df = data.as_dataframe()?;

        // Parse and apply the expression
        // This is a simplified version - in production, you'd want a proper expression parser
        let new_column = if expression.contains("*") {
            // Handle multiplication
            let parts: Vec<&str> = expression.split("*").collect();
            if parts.len() == 2 {
                let column_name = parts[0].trim();
                let factor: f64 = parts[1].trim().parse()?;

                df.column(column_name)?
                    .cast(&DataType::Float64)?
                    .f64()?
                    .apply(|v| v.map(|x| x * factor))
                    .into_series()
            } else {
                anyhow::bail!("Invalid expression: {}", expression);
            }
        } else if expression.contains("+") {
            // Handle addition
            let parts: Vec<&str> = expression.split("+").collect();
            if parts.len() == 2 {
                let column_name = parts[0].trim();
                let addend: f64 = parts[1].trim().parse()?;

                df.column(column_name)?
                    .cast(&DataType::Float64)?
                    .f64()?
                    .apply(|v| v.map(|x| x + addend))
                    .into_series()
            } else {
                anyhow::bail!("Invalid expression: {}", expression);
            }
        } else if expression.contains("-") {
            // Handle subtraction
            let parts: Vec<&str> = expression.split("-").collect();
            if parts.len() == 2 {
                let column_name = parts[0].trim();
                let subtrahend: f64 = parts[1].trim().parse()?;

                df.column(column_name)?
                    .cast(&DataType::Float64)?
                    .f64()?
                    .apply(|v| v.map(|x| x - subtrahend))
                    .into_series()
            } else {
                anyhow::bail!("Invalid expression: {}", expression);
            }
        } else if expression.contains("/") {
            // Handle division
            let parts: Vec<&str> = expression.split("/").collect();
            if parts.len() == 2 {
                // Check if both parts are columns or if one is a number
                if let Ok(divisor) = parts[1].trim().parse::<f64>() {
                    // Column / number
                    let column_name = parts[0].trim();
                    df.column(column_name)?
                        .cast(&DataType::Float64)?
                        .f64()?
                        .apply(|v| v.map(|x| x / divisor))
                        .into_series()
                } else {
                    // Column / column
                    let col1 = parts[0].trim();
                    let col2 = parts[1].trim();
                    let s1 = df
                        .column(col1)?
                        .cast(&DataType::Float64)?
                        .as_materialized_series()
                        .clone();
                    let s2 = df
                        .column(col2)?
                        .cast(&DataType::Float64)?
                        .as_materialized_series()
                        .clone();
                    (&s1 / &s2)?
                }
            } else {
                anyhow::bail!("Invalid expression: {}", expression);
            }
        } else {
            // Assume it's a column name or constant
            // Try parsing as boolean first
            let trimmed_expr = expression.trim();
            if trimmed_expr.eq_ignore_ascii_case("true") {
                Series::new(output_column.into(), vec![true; df.height()])
            } else if trimmed_expr.eq_ignore_ascii_case("false") {
                Series::new(output_column.into(), vec![false; df.height()])
            } else if (trimmed_expr.starts_with('"') && trimmed_expr.ends_with('"'))
                || (trimmed_expr.starts_with('\'') && trimmed_expr.ends_with('\''))
            {
                // String constant (quoted)
                let string_value = &trimmed_expr[1..trimmed_expr.len() - 1];
                Series::new(output_column.into(), vec![string_value; df.height()])
            } else if let Ok(constant) = expression.parse::<f64>() {
                // Try parsing as number
                Series::new(output_column.into(), vec![constant; df.height()])
            } else {
                // Assume it's a column name
                df.column(expression)?.as_materialized_series().clone()
            }
        };

        // Add or replace the column
        let new_column = new_column.with_name(output_column.into());

        let output_col_name: &str = output_column;
        if df
            .get_column_names()
            .iter()
            .any(|name| name.as_str() == output_col_name)
        {
            df.replace(output_column, new_column)?;
        } else {
            df.with_column(new_column)?;
        }

        Ok(DataFormat::DataFrame(df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("expression") {
            anyhow::bail!("Map requires 'expression' configuration");
        }

        if !config.contains_key("output_column") {
            anyhow::bail!("Map requires 'output_column' configuration");
        }

        Ok(())
    }
}
