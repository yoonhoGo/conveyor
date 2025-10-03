use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::traits::{DataFormat, Transform};

pub struct GroupByTransform;

#[async_trait]
impl Transform for GroupByTransform {
    async fn name(&self) -> &str {
        "group_by"
    }

    async fn apply(
        &self,
        data: DataFormat,
        config: &Option<HashMap<String, toml::Value>>,
    ) -> Result<DataFormat> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("GroupBy transform requires configuration"))?;

        // Get group columns (can be single string or array)
        let group_columns: Vec<String> = if let Some(cols) = config.get("by") {
            match cols {
                toml::Value::String(s) => vec![s.clone()],
                toml::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => anyhow::bail!("'by' must be a string or array of strings"),
            }
        } else {
            anyhow::bail!("GroupBy requires 'by' configuration");
        };

        // Get aggregations (array of {column, operation, output_column?})
        let aggs = config
            .get("aggregations")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("GroupBy requires 'aggregations' configuration"))?;

        let df = data.as_dataframe()?;

        // Build aggregation expressions
        let mut agg_exprs = Vec::new();

        for agg in aggs {
            let agg_table = agg
                .as_table()
                .ok_or_else(|| anyhow::anyhow!("Each aggregation must be a table"))?;

            let column = agg_table
                .get("column")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Aggregation requires 'column' field"))?;

            let operation = agg_table
                .get("operation")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Aggregation requires 'operation' field"))?;

            let output_name = agg_table
                .get("output_column")
                .and_then(|v| v.as_str())
                .unwrap_or(column);

            let expr = match operation {
                "sum" => col(column).sum().alias(output_name),
                "avg" | "mean" => col(column).mean().alias(output_name),
                "count" => col(column).count().alias(output_name),
                "min" => col(column).min().alias(output_name),
                "max" => col(column).max().alias(output_name),
                "median" => col(column).median().alias(output_name),
                "std" => col(column).std(1).alias(output_name),
                "var" => col(column).var(1).alias(output_name),
                "first" => col(column).first().alias(output_name),
                "last" => col(column).last().alias(output_name),
                _ => anyhow::bail!(
                    "Unsupported operation '{}'. Supported: sum, avg, count, min, max, median, std, var, first, last",
                    operation
                ),
            };

            agg_exprs.push(expr);
        }

        // Use LazyFrame for groupby
        let result = df
            .lazy()
            .group_by(group_columns.iter().map(col).collect::<Vec<_>>())
            .agg(agg_exprs)
            .collect()?;

        Ok(DataFormat::DataFrame(result))
    }

    async fn validate_config(&self, config: &Option<HashMap<String, toml::Value>>) -> Result<()> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("GroupBy transform requires configuration"))?;

        if !config.contains_key("by") {
            anyhow::bail!("GroupBy requires 'by' configuration");
        }

        if !config.contains_key("aggregations") {
            anyhow::bail!("GroupBy requires 'aggregations' configuration");
        }

        Ok(())
    }
}
