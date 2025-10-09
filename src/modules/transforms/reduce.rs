use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::metadata::{ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct ReduceTransform;

#[async_trait]
impl Stage for ReduceTransform {
    fn name(&self) -> &str {
        "reduce"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert("column".to_string(), toml::Value::String("revenue".to_string()));
        example1.insert("operation".to_string(), toml::Value::String("sum".to_string()));
        example1.insert("output_column".to_string(), toml::Value::String("total_revenue".to_string()));

        let mut example2 = HashMap::new();
        example2.insert("column".to_string(), toml::Value::String("score".to_string()));
        example2.insert("operation".to_string(), toml::Value::String("avg".to_string()));

        StageMetadata::builder("reduce", StageCategory::Transform)
            .description("Reduce column to a single aggregate value")
            .long_description(
                "Reduces an entire column to a single aggregate value using various operations. \
                Supports sum, avg/mean, count, min, max, median, std, var. \
                Returns a single-row DataFrame with the result. \
                Similar to SQL aggregate functions without GROUP BY."
            )
            .parameter(ConfigParameter::required(
                "column",
                ParameterType::String,
                "Column name to aggregate"
            ))
            .parameter(ConfigParameter::required(
                "operation",
                ParameterType::String,
                "Aggregation operation"
            ).with_validation(ParameterValidation::allowed_values([
                "sum", "avg", "mean", "count", "min", "max", "median", "std", "var"
            ])))
            .parameter(ConfigParameter::optional(
                "output_column",
                ParameterType::String,
                "result",
                "Name for the output column"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Calculate total revenue",
                example1,
                Some("Sum all revenue values into total_revenue")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Calculate average score",
                example2,
                Some("Compute the average score across all rows")
            ))
            .tag("reduce")
            .tag("aggregate")
            .tag("summarize")
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
            .ok_or_else(|| anyhow::anyhow!("Reduce transform requires input data"))?;

        let column = config
            .get("column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Reduce requires 'column' configuration"))?;

        let operation = config
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Reduce requires 'operation' configuration (sum, avg, count, min, max, median, std, var)"))?;

        let output_column = config
            .get("output_column")
            .and_then(|v| v.as_str())
            .unwrap_or("result");

        let df = data.as_dataframe()?;

        // Perform aggregation using LazyFrame
        let expr = match operation {
            "sum" => col(column).sum().alias(output_column),
            "avg" | "mean" => col(column).mean().alias(output_column),
            "count" => col(column).count().alias(output_column),
            "min" => col(column).min().alias(output_column),
            "max" => col(column).max().alias(output_column),
            "median" => col(column).median().alias(output_column),
            "std" => col(column).std(1).alias(output_column),
            "var" => col(column).var(1).alias(output_column),
            _ => anyhow::bail!(
                "Unsupported operation '{}'. Supported: sum, avg, count, min, max, median, std, var",
                operation
            ),
        };

        let result_df = df.lazy().select([expr]).collect()?;
        Ok(DataFormat::DataFrame(result_df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("column") {
            anyhow::bail!("Reduce requires 'column' configuration");
        }

        if !config.contains_key("operation") {
            anyhow::bail!("Reduce requires 'operation' configuration");
        }

        let operation = config.get("operation").and_then(|v| v.as_str()).unwrap();
        let valid_ops = [
            "sum", "avg", "mean", "count", "min", "max", "median", "std", "var",
        ];
        if !valid_ops.contains(&operation) {
            anyhow::bail!(
                "Invalid operation '{}'. Must be one of: {}",
                operation,
                valid_ops.join(", ")
            );
        }

        Ok(())
    }
}
