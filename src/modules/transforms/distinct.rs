use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::metadata::{ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct DistinctTransform;

#[async_trait]
impl Stage for DistinctTransform {
    fn name(&self) -> &str {
        "distinct"
    }

    fn metadata(&self) -> StageMetadata {
        let example1 = HashMap::new();

        let mut example2 = HashMap::new();
        example2.insert("columns".to_string(), toml::Value::Array(vec![
            toml::Value::String("user_id".to_string()),
            toml::Value::String("date".to_string()),
        ]));
        example2.insert("keep".to_string(), toml::Value::String("first".to_string()));

        StageMetadata::builder("distinct", StageCategory::Transform)
            .description("Remove duplicate rows from DataFrame")
            .long_description(
                "Removes duplicate rows based on all columns or a subset of columns. \
                Can control which duplicate to keep (first, last, none, any). \
                Similar to SQL DISTINCT or SELECT DISTINCT."
            )
            .parameter(ConfigParameter::optional(
                "columns",
                ParameterType::String,
                "all columns",
                "Column name(s) to check for duplicates (string or array of strings)"
            ))
            .parameter(ConfigParameter::optional(
                "keep",
                ParameterType::String,
                "first",
                "Which duplicate to keep"
            ).with_validation(ParameterValidation::allowed_values([
                "first", "last", "none", "any"
            ])))
            .example(crate::core::metadata::ConfigExample::new(
                "Remove all duplicates",
                example1,
                Some("Keep only unique rows across all columns")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Distinct by specific columns",
                example2,
                Some("Remove duplicates based on user_id and date, keeping first occurrence")
            ))
            .tag("distinct")
            .tag("unique")
            .tag("deduplication")
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
            .ok_or_else(|| anyhow::anyhow!("Distinct transform requires input data"))?;

        let df = data.as_dataframe()?;

        // Get optional subset of columns
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
        let result = if let Some(cols) = subset {
            df.unique::<Vec<String>, &str>(Some(&cols), keep, None)?
        } else {
            df.unique::<Vec<String>, &str>(None, keep, None)?
        };

        Ok(DataFormat::DataFrame(result))
    }

    async fn validate_config(&self, _config: &HashMap<String, toml::Value>) -> Result<()> {
        // Config is optional for distinct
        Ok(())
    }
}
