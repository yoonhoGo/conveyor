use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct ChunkTransform;

#[async_trait]
impl Stage for ChunkTransform {
    fn name(&self) -> &str {
        "chunk"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example_config = HashMap::new();
        example_config.insert("column".to_string(), toml::Value::String("id".to_string()));
        example_config.insert("batch_size".to_string(), toml::Value::Integer(200));
        example_config.insert(
            "output_column".to_string(),
            toml::Value::String("ids".to_string()),
        );

        StageMetadata::builder("chunk", StageCategory::Transform)
            .description("Chunk column values into batches")
            .long_description(
                "Splits rows into chunks of specified size and groups column values into arrays. \
                Useful for batch processing large datasets, creating batch API requests, or \
                grouping IDs for MongoDB $in queries. Each output row contains an array of \
                values from the specified column.",
            )
            .parameter(ConfigParameter::required(
                "column",
                ParameterType::String,
                "Column name to chunk",
            ))
            .parameter(
                ConfigParameter::optional(
                    "batch_size",
                    ParameterType::Integer,
                    "100",
                    "Number of values per chunk",
                )
                .with_validation(ParameterValidation::range(1.0, 10000.0)),
            )
            .parameter(ConfigParameter::optional(
                "output_column",
                ParameterType::String,
                "chunked",
                "Name for the output array column",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Chunk IDs for batch processing",
                example_config,
                Some("Create batches of 200 IDs for batch API calls"),
            ))
            .tag("chunk")
            .tag("batch")
            .tag("array")
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
            .ok_or_else(|| anyhow::anyhow!("Chunk transform requires input data"))?;

        // Get configuration
        let column = config
            .get("column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'column' configuration"))?;

        let batch_size = config
            .get("batch_size")
            .and_then(|v| v.as_integer())
            .unwrap_or(100) as usize;

        if batch_size == 0 {
            anyhow::bail!("batch_size must be greater than 0");
        }

        let output_column = config
            .get("output_column")
            .and_then(|v| v.as_str())
            .unwrap_or("chunked");

        // Convert to RecordBatch for easier processing
        let records = data.as_record_batch()?;

        // Extract column values
        let values: Vec<serde_json::Value> = records
            .iter()
            .filter_map(|record| record.get(column).cloned())
            .collect();

        if values.is_empty() {
            anyhow::bail!("Column '{}' not found or empty", column);
        }

        // Create chunks
        let chunks: Vec<HashMap<String, serde_json::Value>> = values
            .chunks(batch_size)
            .map(|chunk| {
                let mut record = HashMap::new();
                record.insert(
                    output_column.to_string(),
                    serde_json::Value::Array(chunk.to_vec()),
                );
                record
            })
            .collect();

        Ok(DataFormat::RecordBatch(chunks))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("column") {
            anyhow::bail!("Chunk requires 'column' configuration");
        }

        if let Some(batch_size) = config.get("batch_size").and_then(|v| v.as_integer()) {
            if batch_size <= 0 {
                anyhow::bail!("batch_size must be greater than 0");
            }
            if batch_size > 10000 {
                anyhow::bail!("batch_size must be 10000 or less");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_chunk_basic() {
        let transform = ChunkTransform;

        // Create test data
        let records = vec![
            HashMap::from([("id".to_string(), json!("id1"))]),
            HashMap::from([("id".to_string(), json!("id2"))]),
            HashMap::from([("id".to_string(), json!("id3"))]),
            HashMap::from([("id".to_string(), json!("id4"))]),
            HashMap::from([("id".to_string(), json!("id5"))]),
        ];

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::RecordBatch(records));

        let mut config = HashMap::new();
        config.insert("column".to_string(), toml::Value::String("id".to_string()));
        config.insert("batch_size".to_string(), toml::Value::Integer(2));
        config.insert(
            "output_column".to_string(),
            toml::Value::String("ids".to_string()),
        );

        let result = transform.execute(inputs, &config).await.unwrap();
        let output_records = result.as_record_batch().unwrap();

        assert_eq!(output_records.len(), 3); // 5 items / 2 = 3 chunks

        // Check first chunk
        let chunk1 = &output_records[0];
        let ids1 = chunk1.get("ids").unwrap().as_array().unwrap();
        assert_eq!(ids1.len(), 2);
        assert_eq!(ids1[0], json!("id1"));
        assert_eq!(ids1[1], json!("id2"));

        // Check last chunk
        let chunk3 = &output_records[2];
        let ids3 = chunk3.get("ids").unwrap().as_array().unwrap();
        assert_eq!(ids3.len(), 1);
        assert_eq!(ids3[0], json!("id5"));
    }

    #[tokio::test]
    async fn test_chunk_default_batch_size() {
        let transform = ChunkTransform;

        // Create 150 records
        let records: Vec<HashMap<String, serde_json::Value>> = (1..=150)
            .map(|i| HashMap::from([("id".to_string(), json!(format!("id{}", i)))]))
            .collect();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::RecordBatch(records));

        let mut config = HashMap::new();
        config.insert("column".to_string(), toml::Value::String("id".to_string()));
        // No batch_size specified, should default to 100

        let result = transform.execute(inputs, &config).await.unwrap();
        let output_records = result.as_record_batch().unwrap();

        assert_eq!(output_records.len(), 2); // 150 items / 100 = 2 chunks
    }

    #[tokio::test]
    async fn test_chunk_exact_division() {
        let transform = ChunkTransform;

        let records = vec![
            HashMap::from([("id".to_string(), json!("id1"))]),
            HashMap::from([("id".to_string(), json!("id2"))]),
            HashMap::from([("id".to_string(), json!("id3"))]),
            HashMap::from([("id".to_string(), json!("id4"))]),
        ];

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::RecordBatch(records));

        let mut config = HashMap::new();
        config.insert("column".to_string(), toml::Value::String("id".to_string()));
        config.insert("batch_size".to_string(), toml::Value::Integer(2));

        let result = transform.execute(inputs, &config).await.unwrap();
        let output_records = result.as_record_batch().unwrap();

        assert_eq!(output_records.len(), 2); // 4 items / 2 = 2 chunks exactly
    }

    #[tokio::test]
    async fn test_chunk_validation() {
        let transform = ChunkTransform;

        // Missing column
        let config = HashMap::new();
        let result = transform.validate_config(&config).await;
        assert!(result.is_err());

        // Invalid batch_size (0)
        let mut config = HashMap::new();
        config.insert("column".to_string(), toml::Value::String("id".to_string()));
        config.insert("batch_size".to_string(), toml::Value::Integer(0));
        let result = transform.validate_config(&config).await;
        assert!(result.is_err());

        // Invalid batch_size (too large)
        let mut config = HashMap::new();
        config.insert("column".to_string(), toml::Value::String("id".to_string()));
        config.insert("batch_size".to_string(), toml::Value::Integer(10001));
        let result = transform.validate_config(&config).await;
        assert!(result.is_err());

        // Valid config
        let mut config = HashMap::new();
        config.insert("column".to_string(), toml::Value::String("id".to_string()));
        config.insert("batch_size".to_string(), toml::Value::Integer(100));
        let result = transform.validate_config(&config).await;
        assert!(result.is_ok());
    }
}
