use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct MapTransform;

#[async_trait]
impl Stage for MapTransform {
    fn name(&self) -> &str {
        "map"
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        
            

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
