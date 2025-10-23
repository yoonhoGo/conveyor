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
        example_config.insert("batch_size".to_string(), toml::Value::Integer(100));
        example_config.insert(
            "output_column".to_string(),
            toml::Value::String("records".to_string()),
        );

        StageMetadata::builder("chunk", StageCategory::Transform)
            .description("Group rows into batches")
            .long_description(
                "Groups multiple rows into chunks of specified size. Each output row contains \
                an array of original records. Useful for batch processing large datasets, \
                creating batch API requests, or grouping records for bulk operations. \
                The entire row data is preserved in each chunk.",
            )
            .parameter(
                ConfigParameter::optional(
                    "batch_size",
                    ParameterType::Integer,
                    "100",
                    "Number of rows per chunk",
                )
                .with_validation(ParameterValidation::range(1.0, 10000.0)),
            )
            .parameter(ConfigParameter::optional(
                "output_column",
                ParameterType::String,
                "records",
                "Name for the output array column containing chunked records",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Chunk records for batch processing",
                example_config,
                Some("Group 100 rows into each batch for bulk API calls"),
            ))
            .tag("chunk")
            .tag("batch")
            .tag("group")
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
            .unwrap_or("records");

        // Convert to RecordBatch for easier processing
        let records = data.as_record_batch()?;

        if records.is_empty() {
            return Ok(DataFormat::RecordBatch(vec![]));
        }

        // Group records into chunks
        let chunks: Vec<HashMap<String, serde_json::Value>> = records
            .chunks(batch_size)
            .map(|chunk| {
                let mut record = HashMap::new();
                // Convert each record in chunk to JSON value
                let chunk_values: Vec<serde_json::Value> = chunk
                    .iter()
                    .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
                    .collect();
                record.insert(
                    output_column.to_string(),
                    serde_json::Value::Array(chunk_values),
                );
                record
            })
            .collect();

        Ok(DataFormat::RecordBatch(chunks))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
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

        // Create test data with multiple fields
        let records = vec![
            HashMap::from([
                ("id".to_string(), json!("id1")),
                ("name".to_string(), json!("Alice")),
            ]),
            HashMap::from([
                ("id".to_string(), json!("id2")),
                ("name".to_string(), json!("Bob")),
            ]),
            HashMap::from([
                ("id".to_string(), json!("id3")),
                ("name".to_string(), json!("Charlie")),
            ]),
            HashMap::from([
                ("id".to_string(), json!("id4")),
                ("name".to_string(), json!("David")),
            ]),
            HashMap::from([
                ("id".to_string(), json!("id5")),
                ("name".to_string(), json!("Eve")),
            ]),
        ];

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::RecordBatch(records));

        let mut config = HashMap::new();
        config.insert("batch_size".to_string(), toml::Value::Integer(2));
        config.insert(
            "output_column".to_string(),
            toml::Value::String("records".to_string()),
        );

        let result = transform.execute(inputs, &config).await.unwrap();
        let output_records = result.as_record_batch().unwrap();

        assert_eq!(output_records.len(), 3); // 5 items / 2 = 3 chunks

        // Check first chunk
        let chunk1 = &output_records[0];
        let records1 = chunk1.get("records").unwrap().as_array().unwrap();
        assert_eq!(records1.len(), 2);
        assert_eq!(records1[0]["id"], json!("id1"));
        assert_eq!(records1[0]["name"], json!("Alice"));
        assert_eq!(records1[1]["id"], json!("id2"));
        assert_eq!(records1[1]["name"], json!("Bob"));

        // Check last chunk
        let chunk3 = &output_records[2];
        let records3 = chunk3.get("records").unwrap().as_array().unwrap();
        assert_eq!(records3.len(), 1);
        assert_eq!(records3[0]["id"], json!("id5"));
        assert_eq!(records3[0]["name"], json!("Eve"));
    }

    #[tokio::test]
    async fn test_chunk_default_batch_size() {
        let transform = ChunkTransform;

        // Create 150 records
        let records: Vec<HashMap<String, serde_json::Value>> = (1..=150)
            .map(|i| {
                HashMap::from([
                    ("id".to_string(), json!(i)),
                    ("value".to_string(), json!(format!("value{}", i))),
                ])
            })
            .collect();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::RecordBatch(records));

        let config = HashMap::new();
        // No batch_size specified, should default to 100

        let result = transform.execute(inputs, &config).await.unwrap();
        let output_records = result.as_record_batch().unwrap();

        assert_eq!(output_records.len(), 2); // 150 items / 100 = 2 chunks

        // Check first chunk has 100 records
        let chunk1 = &output_records[0];
        let records1 = chunk1.get("records").unwrap().as_array().unwrap();
        assert_eq!(records1.len(), 100);

        // Check second chunk has 50 records
        let chunk2 = &output_records[1];
        let records2 = chunk2.get("records").unwrap().as_array().unwrap();
        assert_eq!(records2.len(), 50);
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
        config.insert("batch_size".to_string(), toml::Value::Integer(2));

        let result = transform.execute(inputs, &config).await.unwrap();
        let output_records = result.as_record_batch().unwrap();

        assert_eq!(output_records.len(), 2); // 4 items / 2 = 2 chunks exactly

        // Verify each chunk has exactly 2 records
        for chunk in &output_records {
            let chunk_records = chunk.get("records").unwrap().as_array().unwrap();
            assert_eq!(chunk_records.len(), 2);
        }
    }

    #[tokio::test]
    async fn test_chunk_validation() {
        let transform = ChunkTransform;

        // Empty config is valid (uses defaults)
        let config = HashMap::new();
        let result = transform.validate_config(&config).await;
        assert!(result.is_ok());

        // Invalid batch_size (0)
        let mut config = HashMap::new();
        config.insert("batch_size".to_string(), toml::Value::Integer(0));
        let result = transform.validate_config(&config).await;
        assert!(result.is_err());

        // Invalid batch_size (too large)
        let mut config = HashMap::new();
        config.insert("batch_size".to_string(), toml::Value::Integer(10001));
        let result = transform.validate_config(&config).await;
        assert!(result.is_err());

        // Valid config
        let mut config = HashMap::new();
        config.insert("batch_size".to_string(), toml::Value::Integer(100));
        let result = transform.validate_config(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chunk_empty_input() {
        let transform = ChunkTransform;

        let records: Vec<HashMap<String, serde_json::Value>> = vec![];

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::RecordBatch(records));

        let mut config = HashMap::new();
        config.insert("batch_size".to_string(), toml::Value::Integer(10));

        let result = transform.execute(inputs, &config).await.unwrap();
        let output_records = result.as_record_batch().unwrap();

        assert_eq!(output_records.len(), 0); // Empty input should produce empty output
    }
}
