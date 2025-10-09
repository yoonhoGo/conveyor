use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::metadata::{ConfigParameter, ParameterType, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct GroupByTransform;

#[async_trait]
impl Stage for GroupByTransform {
    fn name(&self) -> &str {
        "group_by"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example_config = HashMap::new();
        example_config.insert("by".to_string(), toml::Value::Array(vec![
            toml::Value::String("department".to_string()),
        ]));

        let mut agg1 = toml::map::Map::new();
        agg1.insert("column".to_string(), toml::Value::String("salary".to_string()));
        agg1.insert("operation".to_string(), toml::Value::String("avg".to_string()));
        agg1.insert("output_column".to_string(), toml::Value::String("avg_salary".to_string()));

        let mut agg2 = toml::map::Map::new();
        agg2.insert("column".to_string(), toml::Value::String("employee_id".to_string()));
        agg2.insert("operation".to_string(), toml::Value::String("count".to_string()));
        agg2.insert("output_column".to_string(), toml::Value::String("employee_count".to_string()));

        example_config.insert("aggregations".to_string(), toml::Value::Array(vec![
            toml::Value::Table(agg1),
            toml::Value::Table(agg2),
        ]));

        StageMetadata::builder("group_by", StageCategory::Transform)
            .description("Group rows and apply aggregations")
            .long_description(
                "Groups DataFrame rows by one or more columns and applies aggregation functions. \
                Similar to SQL GROUP BY. Supports multiple aggregations: sum, avg/mean, count, min, max, \
                median, std, var, first, last. Each aggregation can specify an output column name."
            )
            .parameter(ConfigParameter::required(
                "by",
                ParameterType::String,
                "Column name(s) to group by (string or array of strings)"
            ))
            .parameter(ConfigParameter::required(
                "aggregations",
                ParameterType::String,
                "Array of aggregation specifications (each with column, operation, and optional output_column)"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Group by department",
                example_config,
                Some("Calculate average salary and employee count per department")
            ))
            .tag("group_by")
            .tag("aggregation")
            .tag("sql")
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
            .ok_or_else(|| anyhow::anyhow!("GroupBy transform requires input data"))?;

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

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("by") {
            anyhow::bail!("GroupBy requires 'by' configuration");
        }

        if !config.contains_key("aggregations") {
            anyhow::bail!("GroupBy requires 'aggregations' configuration");
        }

        Ok(())
    }
}
