use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::traits::{DataFormat, Transform};

pub struct DistinctTransform;

#[async_trait]
impl Transform for DistinctTransform {
    async fn name(&self) -> &str {
        "distinct"
    }

    async fn apply(
        &self,
        data: DataFormat,
        config: &Option<HashMap<String, toml::Value>>,
    ) -> Result<DataFormat> {
        let df = data.as_dataframe()?;

        // Get optional subset of columns and keep strategy
        let result = if let Some(config) = config {
            let subset = if let Some(cols) = config.get("columns") {
                Some(match cols {
                    toml::Value::String(s) => vec![s.clone()],
                    toml::Value::Array(arr) => arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                    _ => anyhow::bail!("'columns' must be a string or array of strings"),
                })
            } else {
                None
            };

            // Get keep strategy
            let keep = if let Some(keep_val) = config.get("keep") {
                match keep_val.as_str() {
                    Some("first") => UniqueKeepStrategy::First,
                    Some("last") => UniqueKeepStrategy::Last,
                    Some("none") => UniqueKeepStrategy::None,
                    Some("any") => UniqueKeepStrategy::Any,
                    _ => anyhow::bail!("'keep' must be one of: first, last, none, any"),
                }
            } else {
                UniqueKeepStrategy::First
            };

            // Use unique with proper type specification
            if let Some(cols) = subset {
                df.unique::<Vec<String>, &str>(Some(&cols), keep, None)?
            } else {
                df.unique::<Vec<String>, &str>(None, keep, None)?
            }
        } else {
            // No config, use all columns with default keep strategy
            df.unique::<Vec<String>, &str>(None, UniqueKeepStrategy::First, None)?
        };

        Ok(DataFormat::DataFrame(result))
    }

    async fn validate_config(&self, _config: &Option<HashMap<String, toml::Value>>) -> Result<()> {
        // Config is optional for distinct
        Ok(())
    }
}
