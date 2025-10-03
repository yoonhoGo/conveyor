use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::traits::{DataFormat, Transform};

pub struct ReduceTransform;

#[async_trait]
impl Transform for ReduceTransform {
    async fn name(&self) -> &str {
        "reduce"
    }

    async fn apply(
        &self,
        data: DataFormat,
        config: &Option<HashMap<String, toml::Value>>,
    ) -> Result<DataFormat> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Reduce transform requires configuration"))?;

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

    async fn validate_config(&self, config: &Option<HashMap<String, toml::Value>>) -> Result<()> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Reduce transform requires configuration"))?;

        if !config.contains_key("column") {
            anyhow::bail!("Reduce requires 'column' configuration");
        }

        if !config.contains_key("operation") {
            anyhow::bail!("Reduce requires 'operation' configuration");
        }

        let operation = config.get("operation").and_then(|v| v.as_str()).unwrap();
        let valid_ops = ["sum", "avg", "mean", "count", "min", "max", "median", "std", "var"];
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
