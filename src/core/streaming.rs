use anyhow::Result;
use futures::StreamExt;
use polars::prelude::*;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::pin::Pin;
use tokio_stream::Stream;

use crate::core::traits::RecordBatch;

/// Micro-batching utilities for stream processing
pub struct StreamBatcher {
    batch_size: usize,
}

impl StreamBatcher {
    pub fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }

    /// Convert a stream of individual records into batches
    pub fn batch_records(
        &self,
        stream: Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>,
    ) -> Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>> {
        let batch_size = self.batch_size;

        Box::pin(stream.ready_chunks(batch_size).map(move |chunks| {
            // Combine all successful batches into one
            let mut combined = Vec::new();
            for chunk_result in chunks {
                match chunk_result {
                    Ok(batch) => combined.extend(batch),
                    Err(e) => return Err(e),
                }
            }
            Ok(combined)
        }))
    }

    /// Collect a stream into a complete RecordBatch
    pub async fn collect_stream(
        stream: Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>,
    ) -> Result<RecordBatch> {
        let batches: Vec<RecordBatch> = stream
            .collect::<Vec<Result<RecordBatch>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        Ok(batches.into_iter().flatten().collect())
    }

    /// Convert a RecordBatch stream into a DataFrame
    pub async fn stream_to_dataframe(
        stream: Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>,
    ) -> Result<DataFrame> {
        let records = Self::collect_stream(stream).await?;

        if records.is_empty() {
            return Ok(DataFrame::empty());
        }

        // Convert to JSON and parse with Polars
        let json_str = serde_json::to_string(&records)?;
        let cursor = std::io::Cursor::new(json_str.as_bytes());
        let df = JsonReader::new(cursor).finish()?;

        Ok(df)
    }
}

/// Window configuration for stream processing
#[derive(Debug, Clone)]
pub enum WindowType {
    /// Fixed-size tumbling window
    Tumbling { size: usize },
    /// Sliding window with overlap
    Sliding { size: usize, slide: usize },
    /// Session window with timeout
    Session { gap: std::time::Duration },
}

/// Windowing utilities for stream processing
pub struct StreamWindower {
    window_type: WindowType,
}

impl StreamWindower {
    pub fn new(window_type: WindowType) -> Self {
        Self { window_type }
    }

    /// Apply windowing to a stream
    pub fn apply(
        &self,
        stream: Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>,
    ) -> Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>> {
        match &self.window_type {
            WindowType::Tumbling { size } => {
                let window_size = *size;
                Box::pin(stream.ready_chunks(window_size).map(move |chunks| {
                    let mut combined = Vec::new();
                    for chunk_result in chunks {
                        match chunk_result {
                            Ok(batch) => combined.extend(batch),
                            Err(e) => return Err(e),
                        }
                    }
                    Ok(combined)
                }))
            }
            WindowType::Sliding { size: _, slide } => {
                // TODO: Implement sliding window
                // For now, use tumbling window with slide size
                let slide_size = *slide;
                Box::pin(stream.ready_chunks(slide_size).map(move |chunks| {
                    let mut combined = Vec::new();
                    for chunk_result in chunks {
                        match chunk_result {
                            Ok(batch) => combined.extend(batch),
                            Err(e) => return Err(e),
                        }
                    }
                    Ok(combined)
                }))
            }
            WindowType::Session { gap: _ } => {
                // TODO: Implement session window
                // For now, pass through
                stream
            }
        }
    }
}

/// Stream processing utilities
pub struct StreamProcessor;

impl StreamProcessor {
    /// Create a stream from a single RecordBatch
    pub fn from_batch(
        batch: RecordBatch,
    ) -> Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>> {
        Box::pin(tokio_stream::once(Ok(batch)))
    }

    /// Create a stream from a DataFrame
    pub fn from_dataframe(
        df: DataFrame,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>> {
        // Convert DataFrame to RecordBatch
        let mut records = Vec::new();
        let height = df.height();

        for i in 0..height {
            let mut record = HashMap::new();
            for col in df.get_columns() {
                let name = col.name().to_string();
                let value = match col.dtype() {
                    DataType::String => {
                        let s = col.str()?;
                        JsonValue::String(s.get(i).unwrap_or("").to_string())
                    }
                    DataType::Int64 => {
                        let s = col.i64()?;
                        s.get(i)
                            .map(|v| JsonValue::Number(v.into()))
                            .unwrap_or(JsonValue::Null)
                    }
                    DataType::Float64 => {
                        let s = col.f64()?;
                        s.get(i)
                            .and_then(|v| serde_json::Number::from_f64(v).map(JsonValue::Number))
                            .unwrap_or(JsonValue::Null)
                    }
                    DataType::Boolean => {
                        let s = col.bool()?;
                        s.get(i).map(JsonValue::Bool).unwrap_or(JsonValue::Null)
                    }
                    _ => JsonValue::Null,
                };
                record.insert(name, value);
            }
            records.push(record);
        }

        Ok(Box::pin(tokio_stream::once(Ok(records))))
    }

    /// Map a function over each record in a stream
    pub fn map<F>(
        stream: Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>,
        f: F,
    ) -> Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>
    where
        F: Fn(RecordBatch) -> Result<RecordBatch> + Send + 'static,
    {
        Box::pin(stream.map(move |batch_result| batch_result.and_then(&f)))
    }

    /// Filter records in a stream
    pub fn filter<F>(
        stream: Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>,
        predicate: F,
    ) -> Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>
    where
        F: Fn(&HashMap<String, JsonValue>) -> bool + Send + 'static,
    {
        Box::pin(stream.map(move |batch_result| {
            batch_result.map(|batch| {
                batch
                    .into_iter()
                    .filter(|record| predicate(record))
                    .collect()
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_batch_records() {
        let records: Vec<Result<RecordBatch>> = vec![
            Ok(vec![HashMap::from([(
                "id".to_string(),
                JsonValue::from(1),
            )])]),
            Ok(vec![HashMap::from([(
                "id".to_string(),
                JsonValue::from(2),
            )])]),
            Ok(vec![HashMap::from([(
                "id".to_string(),
                JsonValue::from(3),
            )])]),
        ];

        let stream = Box::pin(tokio_stream::iter(records));
        let batcher = StreamBatcher::new(2);
        let batched = batcher.batch_records(stream);

        let result: Vec<_> = batched.collect().await;
        assert_eq!(result.len(), 2); // 2 batches (2 + 1)
    }

    #[tokio::test]
    async fn test_collect_stream() {
        let records: Vec<Result<RecordBatch>> = vec![
            Ok(vec![HashMap::from([(
                "id".to_string(),
                JsonValue::from(1),
            )])]),
            Ok(vec![HashMap::from([(
                "id".to_string(),
                JsonValue::from(2),
            )])]),
        ];

        let stream = Box::pin(tokio_stream::iter(records));
        let collected = StreamBatcher::collect_stream(stream).await.unwrap();

        assert_eq!(collected.len(), 2);
    }

    #[tokio::test]
    async fn test_stream_processor_from_batch() {
        let batch = vec![HashMap::from([("id".to_string(), JsonValue::from(1))])];
        let stream = StreamProcessor::from_batch(batch);

        let result: Vec<_> = stream.collect().await;
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_stream_processor_filter() {
        let records: Vec<Result<RecordBatch>> = vec![Ok(vec![
            HashMap::from([("value".to_string(), JsonValue::from(10))]),
            HashMap::from([("value".to_string(), JsonValue::from(20))]),
            HashMap::from([("value".to_string(), JsonValue::from(5))]),
        ])];

        let stream = Box::pin(tokio_stream::iter(records));
        let filtered = StreamProcessor::filter(stream, |record| {
            if let Some(JsonValue::Number(n)) = record.get("value") {
                n.as_i64().unwrap_or(0) >= 10
            } else {
                false
            }
        });

        let result: Vec<_> = filtered.collect().await;
        assert_eq!(result.len(), 1);
        let batch = result[0].as_ref().unwrap();
        assert_eq!(batch.len(), 2); // Only values >= 10
    }
}
