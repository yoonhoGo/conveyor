use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::metadata::{ConfigParameter, ParameterType, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct SortTransform;

#[async_trait]
impl Stage for SortTransform {
    fn name(&self) -> &str {
        "sort"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert("by".to_string(), toml::Value::String("age".to_string()));

        let mut example2 = HashMap::new();
        example2.insert("by".to_string(), toml::Value::Array(vec![
            toml::Value::String("department".to_string()),
            toml::Value::String("salary".to_string()),
        ]));
        example2.insert("descending".to_string(), toml::Value::Array(vec![
            toml::Value::Boolean(false),
            toml::Value::Boolean(true),
        ]));

        StageMetadata::builder("sort", StageCategory::Transform)
            .description("Sort DataFrame rows")
            .long_description(
                "Sorts DataFrame rows by one or more columns in ascending or descending order. \
                Supports multi-column sorting with per-column sort direction. \
                Can control null value placement (first or last)."
            )
            .parameter(ConfigParameter::required(
                "by",
                ParameterType::String,
                "Column name(s) to sort by (string or array of strings)"
            ))
            .parameter(ConfigParameter::optional(
                "descending",
                ParameterType::Boolean,
                "false",
                "Sort in descending order (boolean or array matching 'by' length)"
            ))
            .parameter(ConfigParameter::optional(
                "nulls_last",
                ParameterType::Boolean,
                "false",
                "Place null values last instead of first"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Simple sort",
                example1,
                Some("Sort by age in ascending order")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Multi-column sort",
                example2,
                Some("Sort by department (asc), then salary (desc)")
            ))
            .tag("sort")
            .tag("order")
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
            .ok_or_else(|| anyhow::anyhow!("Sort transform requires input data"))?;

        // Get columns to sort by (can be single string or array)
        let sort_columns: Vec<String> = if let Some(cols) = config.get("by") {
            match cols {
                toml::Value::String(s) => vec![s.clone()],
                toml::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => anyhow::bail!("'by' must be a string or array of strings"),
            }
        } else {
            anyhow::bail!("Sort requires 'by' configuration");
        };

        // Get descending flag(s)
        let descending: Vec<bool> = if let Some(desc) = config.get("descending") {
            match desc {
                toml::Value::Boolean(b) => vec![*b; sort_columns.len()],
                toml::Value::Array(arr) => {
                    let bools: Vec<bool> = arr.iter().filter_map(|v| v.as_bool()).collect();
                    if bools.len() != sort_columns.len() {
                        anyhow::bail!(
                            "If 'descending' is an array, it must have the same length as 'by'"
                        );
                    }
                    bools
                }
                _ => anyhow::bail!("'descending' must be a boolean or array of booleans"),
            }
        } else {
            vec![false; sort_columns.len()]
        };

        // Get nulls_last flag
        let nulls_last = config
            .get("nulls_last")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let df = data.as_dataframe()?;

        // Build sort options
        let sort_options = SortMultipleOptions::default()
            .with_order_descending_multi(descending)
            .with_nulls_last(nulls_last);

        let result = df.sort(&sort_columns, sort_options)?;
        Ok(DataFormat::DataFrame(result))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("by") {
            anyhow::bail!("Sort requires 'by' configuration");
        }

        Ok(())
    }
}
