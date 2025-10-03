use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::traits::{DataFormat, Transform};

pub struct SortTransform;

#[async_trait]
impl Transform for SortTransform {
    async fn name(&self) -> &str {
        "sort"
    }

    async fn apply(
        &self,
        data: DataFormat,
        config: &Option<HashMap<String, toml::Value>>,
    ) -> Result<DataFormat> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sort transform requires configuration"))?;

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
                    let bools: Vec<bool> = arr
                        .iter()
                        .filter_map(|v| v.as_bool())
                        .collect();
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

    async fn validate_config(&self, config: &Option<HashMap<String, toml::Value>>) -> Result<()> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sort transform requires configuration"))?;

        if !config.contains_key("by") {
            anyhow::bail!("Sort requires 'by' configuration");
        }

        Ok(())
    }
}
