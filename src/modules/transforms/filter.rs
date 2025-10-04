use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct FilterTransform;

#[async_trait]
impl Stage for FilterTransform {
    fn name(&self) -> &str {
        "filter.apply"
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Filter transform requires input data"))?;

        let column = config
            .get("column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Filter requires 'column' configuration"))?;

        let operator = config
            .get("operator")
            .and_then(|v| v.as_str())
            .unwrap_or("==");

        let value = config
            .get("value")
            .ok_or_else(|| anyhow::anyhow!("Filter requires 'value' configuration"))?;

        let df = data.as_dataframe()?;

        let filtered_df = match operator {
            "==" | "=" => {
                let expr = col(column).eq(lit(value_to_literal(value)?));
                df.lazy().filter(expr).collect()?
            }
            "!=" | "<>" => {
                let expr = col(column).neq(lit(value_to_literal(value)?));
                df.lazy().filter(expr).collect()?
            }
            ">" => {
                let expr = col(column).gt(lit(value_to_literal(value)?));
                df.lazy().filter(expr).collect()?
            }
            ">=" => {
                let expr = col(column).gt_eq(lit(value_to_literal(value)?));
                df.lazy().filter(expr).collect()?
            }
            "<" => {
                let expr = col(column).lt(lit(value_to_literal(value)?));
                df.lazy().filter(expr).collect()?
            }
            "<=" => {
                let expr = col(column).lt_eq(lit(value_to_literal(value)?));
                df.lazy().filter(expr).collect()?
            }
            "contains" => {
                if let Some(pattern) = value.as_str() {
                    // Filter manually by checking if the string contains the pattern
                    let column_data = df.column(column)?.str()?;
                    let mask: BooleanChunked = column_data
                        .into_iter()
                        .map(|opt_s| opt_s.map(|s| s.contains(pattern)).unwrap_or(false))
                        .collect();
                    df.filter(&mask)?
                } else {
                    anyhow::bail!("'contains' operator requires string value");
                }
            }
            "in" => {
                if let Some(array) = value.as_array() {
                    let values: Vec<String> = array
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();

                    // Filter rows where column value is in the values list
                    let mut masks = Vec::new();
                    for val in &values {
                        masks.push(col(column).eq(lit(val.clone())));
                    }

                    let combined = masks
                        .into_iter()
                        .reduce(|acc, mask| acc.or(mask))
                        .unwrap_or_else(|| lit(false));

                    df.lazy().filter(combined).collect()?
                } else {
                    anyhow::bail!("'in' operator requires array value");
                }
            }
            _ => anyhow::bail!("Unknown filter operator: {}", operator),
        };

        Ok(DataFormat::DataFrame(filtered_df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("column") {
            anyhow::bail!("Filter requires 'column' configuration");
        }

        if !config.contains_key("value") {
            anyhow::bail!("Filter requires 'value' configuration");
        }

        if let Some(operator) = config.get("operator").and_then(|v| v.as_str()) {
            let valid_operators = [
                "==", "=", "!=", "<>", ">", ">=", "<", "<=", "contains", "in",
            ];
            if !valid_operators.contains(&operator) {
                anyhow::bail!(
                    "Invalid filter operator: {}. Must be one of: {:?}",
                    operator,
                    valid_operators
                );
            }
        }

        Ok(())
    }
}

fn value_to_literal(value: &toml::Value) -> Result<LiteralValue> {
    match value {
        toml::Value::String(s) => Ok(LiteralValue::String(s.clone().into())),
        toml::Value::Integer(i) => Ok(LiteralValue::Int64(*i)),
        toml::Value::Float(f) => Ok(LiteralValue::Float64(*f)),
        toml::Value::Boolean(b) => Ok(LiteralValue::Boolean(*b)),
        _ => anyhow::bail!("Unsupported value type for filter"),
    }
}
