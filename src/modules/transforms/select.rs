use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct SelectTransform;

#[async_trait]
impl Stage for SelectTransform {
    fn name(&self) -> &str {
        "select"
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        
            

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
