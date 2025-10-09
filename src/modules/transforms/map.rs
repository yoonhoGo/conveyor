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
        example1.insert("expression".to_string(), toml::Value::String("price * 1.1".to_string()));
        example1.insert("output_column".to_string(), toml::Value::String("price_with_tax".to_string()));

        let mut example2 = HashMap::new();
        example2.insert("expression".to_string(), toml::Value::String("quantity * price".to_string()));
        example2.insert("output_column".to_string(), toml::Value::String("total".to_string()));

        StageMetadata::builder("map", StageCategory::Transform)
            .description("Apply mathematical expressions to create new columns")
            .long_description(
                "Creates new columns by applying mathematical expressions to existing columns. \
                Supports basic arithmetic operations: +, -, *, /. \
                Can operate on single columns with constants or between two columns. \
                Useful for calculated fields and data transformations."
            )
            .parameter(ConfigParameter::required(
                "expression",
                ParameterType::String,
                "Mathematical expression (e.g., 'column * 2', 'col1 + col2')"
            ))
            .parameter(ConfigParameter::required(
                "output_column",
                ParameterType::String,
                "Name of the new column to create"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Add tax calculation",
                example1,
                Some("Create price_with_tax column by multiplying price by 1.1")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Calculate total",
                example2,
                Some("Multiply quantity by price to get total")
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
            if let Ok(constant) = expression.parse::<f64>() {
                Series::new(output_column.into(), vec![constant; df.height()])
            } else {
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
