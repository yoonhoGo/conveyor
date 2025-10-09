use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::core::metadata::{ConfigParameter, ParameterType, StageCategory, StageMetadata};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct SelectTransform;

#[async_trait]
impl Stage for SelectTransform {
    fn name(&self) -> &str {
        "select"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example_config = HashMap::new();
        example_config.insert(
            "columns".to_string(),
            toml::Value::Array(vec![
                toml::Value::String("id".to_string()),
                toml::Value::String("name".to_string()),
                toml::Value::String("email".to_string()),
            ]),
        );

        StageMetadata::builder("select", StageCategory::Transform)
            .description("Select specific columns from DataFrame")
            .long_description(
                "Selects a subset of columns from the input DataFrame, similar to SQL SELECT. \
                Can specify columns as a single string or an array of strings. \
                Useful for reducing data size and focusing on relevant fields.",
            )
            .parameter(ConfigParameter::required(
                "columns",
                ParameterType::String,
                "Column name(s) to select (string or array of strings)",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Select multiple columns",
                example_config,
                Some("Keep only id, name, and email columns"),
            ))
            .tag("select")
            .tag("columns")
            .tag("projection")
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
            .ok_or_else(|| anyhow::anyhow!("Select transform requires input data"))?;

        // Get columns to select
        let columns: Vec<String> = if let Some(cols) = config.get("columns") {
            match cols {
                toml::Value::String(s) => vec![s.clone()],
                toml::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => anyhow::bail!("'columns' must be a string or array of strings"),
            }
        } else {
            anyhow::bail!("Select requires 'columns' configuration");
        };

        let df = data.as_dataframe()?;

        // Select specified columns
        let result = df.select(&columns)?;
        Ok(DataFormat::DataFrame(result))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("columns") {
            anyhow::bail!("Select requires 'columns' configuration");
        }

        Ok(())
    }
}
