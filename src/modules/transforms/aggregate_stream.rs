use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tracing::info;

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::streaming::StreamProcessor;
use crate::core::traits::{DataFormat, RecordBatch};

/// Aggregate stream transform for real-time aggregation
pub struct AggregateStreamTransform;

impl Default for AggregateStreamTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl AggregateStreamTransform {
    pub fn new() -> Self {
        Self
    }

    /// Aggregate a batch of records
    fn aggregate_batch(
        batch: RecordBatch,
        operation: &str,
        group_by: &[String],
        value_column: Option<&str>,
    ) -> Result<RecordBatch> {
        if group_by.is_empty() {
            // Global aggregation (no grouping)
            Self::aggregate_global(batch, operation, value_column)
        } else {
            // Group-by aggregation
            Self::aggregate_grouped(batch, operation, group_by, value_column)
        }
    }

    /// Global aggregation (no grouping)
    fn aggregate_global(
        batch: RecordBatch,
        operation: &str,
        value_column: Option<&str>,
    ) -> Result<RecordBatch> {
        let mut result = HashMap::new();

        match operation {
            "count" => {
                result.insert(
                    "count".to_string(),
                    JsonValue::Number((batch.len() as i64).into()),
                );
            }
            "sum" => {
                let col = value_column
                    .ok_or_else(|| anyhow::anyhow!("sum operation requires 'value_column'"))?;

                let sum: f64 = batch
                    .iter()
                    .filter_map(|record| record.get(col))
                    .filter_map(|v| v.as_f64())
                    .sum();

                result.insert(
                    "sum".to_string(),
                    JsonValue::Number(serde_json::Number::from_f64(sum).unwrap_or(0.into())),
                );
            }
            "avg" => {
                let col = value_column
                    .ok_or_else(|| anyhow::anyhow!("avg operation requires 'value_column'"))?;

                let values: Vec<f64> = batch
                    .iter()
                    .filter_map(|record| record.get(col))
                    .filter_map(|v| v.as_f64())
                    .collect();

                let avg = if values.is_empty() {
                    0.0
                } else {
                    values.iter().sum::<f64>() / values.len() as f64
                };

                result.insert(
                    "avg".to_string(),
                    JsonValue::Number(serde_json::Number::from_f64(avg).unwrap_or(0.into())),
                );
            }
            "min" => {
                let col = value_column
                    .ok_or_else(|| anyhow::anyhow!("min operation requires 'value_column'"))?;

                let min = batch
                    .iter()
                    .filter_map(|record| record.get(col))
                    .filter_map(|v| v.as_f64())
                    .min_by(|a, b| a.partial_cmp(b).unwrap());

                if let Some(min_val) = min {
                    result.insert(
                        "min".to_string(),
                        JsonValue::Number(
                            serde_json::Number::from_f64(min_val).unwrap_or(0.into()),
                        ),
                    );
                }
            }
            "max" => {
                let col = value_column
                    .ok_or_else(|| anyhow::anyhow!("max operation requires 'value_column'"))?;

                let max = batch
                    .iter()
                    .filter_map(|record| record.get(col))
                    .filter_map(|v| v.as_f64())
                    .max_by(|a, b| a.partial_cmp(b).unwrap());

                if let Some(max_val) = max {
                    result.insert(
                        "max".to_string(),
                        JsonValue::Number(
                            serde_json::Number::from_f64(max_val).unwrap_or(0.into()),
                        ),
                    );
                }
            }
            _ => {
                anyhow::bail!("Unsupported aggregation operation: {}", operation);
            }
        }

        Ok(vec![result])
    }

    /// Grouped aggregation
    fn aggregate_grouped(
        batch: RecordBatch,
        operation: &str,
        group_by: &[String],
        value_column: Option<&str>,
    ) -> Result<RecordBatch> {
        // Group records by key
        let mut groups: HashMap<Vec<JsonValue>, Vec<HashMap<String, JsonValue>>> = HashMap::new();

        for record in batch {
            let key: Vec<JsonValue> = group_by
                .iter()
                .filter_map(|col| record.get(col).cloned())
                .collect();

            groups.entry(key).or_default().push(record);
        }

        // Aggregate each group
        let mut results = Vec::new();

        for (key, group_records) in groups {
            let agg_result = Self::aggregate_global(group_records, operation, value_column)?;

            if let Some(mut agg_record) = agg_result.into_iter().next() {
                // Add group-by keys to result
                for (i, col) in group_by.iter().enumerate() {
                    if let Some(key_val) = key.get(i) {
                        agg_record.insert(col.clone(), key_val.clone());
                    }
                }
                results.push(agg_record);
            }
        }

        Ok(results)
    }
}

#[async_trait]
impl Stage for AggregateStreamTransform {
    fn name(&self) -> &str {
        "aggregate_stream"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert(
            "operation".to_string(),
            toml::Value::String("count".to_string()),
        );

        let mut example2 = HashMap::new();
        example2.insert(
            "operation".to_string(),
            toml::Value::String("avg".to_string()),
        );
        example2.insert(
            "group_by".to_string(),
            toml::Value::Array(vec![toml::Value::String("status".to_string())]),
        );
        example2.insert(
            "value_column".to_string(),
            toml::Value::String("response_time".to_string()),
        );

        StageMetadata::builder("aggregate_stream", StageCategory::Transform)
            .description("Apply real-time aggregations to streaming data")
            .long_description(
                "Performs aggregations on streaming or batch data in real-time. \
                Supports count, sum, avg, min, max operations. \
                Can group by one or more columns. \
                Processes data as it arrives for low-latency analytics. \
                Works with Stream, RecordBatch, and DataFrame formats.",
            )
            .parameter(
                ConfigParameter::required(
                    "operation",
                    ParameterType::String,
                    "Aggregation operation to perform",
                )
                .with_validation(ParameterValidation::allowed_values([
                    "count", "sum", "avg", "min", "max",
                ])),
            )
            .parameter(ConfigParameter::optional(
                "group_by",
                ParameterType::String,
                "none",
                "Column name(s) to group by (string or array of strings)",
            ))
            .parameter(ConfigParameter::optional(
                "value_column",
                ParameterType::String,
                "none",
                "Column to aggregate (required for sum, avg, min, max operations)",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Count all records",
                example1,
                Some("Count total number of records in stream"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Average by group",
                example2,
                Some("Calculate average response time grouped by status"),
            ))
            .tag("aggregate")
            .tag("stream")
            .tag("real-time")
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
            .ok_or_else(|| anyhow::anyhow!("Aggregate stream requires input data"))?;

        let operation = config
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'operation' parameter"))?;

        let group_by: Vec<String> = config
            .get("group_by")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let value_column = config.get("value_column").and_then(|v| v.as_str());

        info!(
            "Applying stream aggregation: operation={}, group_by={:?}",
            operation, group_by
        );

        match data {
            DataFormat::Stream(stream) => {
                let op = operation.to_string();
                let groups = group_by.clone();
                let val_col = value_column.map(|s| s.to_string());

                // Apply aggregation to each batch in the stream
                let aggregated = StreamProcessor::map(stream, move |batch| {
                    Self::aggregate_batch(batch, &op, &groups, val_col.as_deref())
                });

                Ok(DataFormat::Stream(aggregated))
            }
            DataFormat::RecordBatch(batch) => {
                let result = Self::aggregate_batch(batch, operation, &group_by, value_column)?;
                Ok(DataFormat::RecordBatch(result))
            }
            DataFormat::DataFrame(df) => {
                // Convert to RecordBatch, aggregate, convert back
                let batch = DataFormat::DataFrame(df).as_record_batch()?;
                let result = Self::aggregate_batch(batch, operation, &group_by, value_column)?;
                Ok(DataFormat::RecordBatch(result))
            }
            DataFormat::Raw(_) => {
                anyhow::bail!("Aggregate stream transform does not support raw data format")
            }
        }
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        // Validate operation
        if !config.contains_key("operation") {
            anyhow::bail!("Missing required 'operation' parameter");
        }

        let operation = config.get("operation").and_then(|v| v.as_str()).unwrap();
        let valid_operations = ["count", "sum", "avg", "min", "max"];
        if !valid_operations.contains(&operation) {
            anyhow::bail!(
                "Invalid operation: {}. Must be one of: {:?}",
                operation,
                valid_operations
            );
        }

        // Validate value_column for operations that need it
        if ["sum", "avg", "min", "max"].contains(&operation) && !config.contains_key("value_column")
        {
            anyhow::bail!(
                "Operation '{}' requires 'value_column' parameter",
                operation
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_aggregate_global_count() {
        let batch = vec![
            HashMap::from([("name".to_string(), json!("Alice"))]),
            HashMap::from([("name".to_string(), json!("Bob"))]),
            HashMap::from([("name".to_string(), json!("Charlie"))]),
        ];

        let result = AggregateStreamTransform::aggregate_global(batch, "count", None).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get("count").unwrap(), &json!(3));
    }

    #[test]
    fn test_aggregate_global_sum() {
        let batch = vec![
            HashMap::from([("value".to_string(), json!(10.0))]),
            HashMap::from([("value".to_string(), json!(20.0))]),
            HashMap::from([("value".to_string(), json!(30.0))]),
        ];

        let result =
            AggregateStreamTransform::aggregate_global(batch, "sum", Some("value")).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get("sum").unwrap(), &json!(60.0));
    }

    #[test]
    fn test_aggregate_grouped_count() {
        let batch = vec![
            HashMap::from([
                ("level".to_string(), json!("error")),
                ("message".to_string(), json!("msg1")),
            ]),
            HashMap::from([
                ("level".to_string(), json!("error")),
                ("message".to_string(), json!("msg2")),
            ]),
            HashMap::from([
                ("level".to_string(), json!("info")),
                ("message".to_string(), json!("msg3")),
            ]),
        ];

        let group_by = vec!["level".to_string()];
        let result =
            AggregateStreamTransform::aggregate_grouped(batch, "count", &group_by, None).unwrap();

        assert_eq!(result.len(), 2); // Two groups: error and info
    }

    #[tokio::test]
    async fn test_validate_config() {
        let transform = AggregateStreamTransform::new();

        let mut config = HashMap::new();
        config.insert(
            "operation".to_string(),
            toml::Value::String("count".to_string()),
        );

        assert!(transform.validate_config(&config).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_missing_value_column() {
        let transform = AggregateStreamTransform::new();

        let mut config = HashMap::new();
        config.insert(
            "operation".to_string(),
            toml::Value::String("sum".to_string()),
        );

        assert!(transform.validate_config(&config).await.is_err());
    }
}
