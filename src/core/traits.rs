use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;

pub type DataFrameResult = Result<DataFrame>;
pub type RecordBatch = Vec<HashMap<String, JsonValue>>;
pub type RecordBatchResult = Result<RecordBatch>;

pub enum DataFormat {
    DataFrame(DataFrame),
    RecordBatch(RecordBatch),
    Raw(Vec<u8>),
    Stream(Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>),
}

impl DataFormat {
    pub fn as_dataframe(&self) -> Result<DataFrame> {
        match self {
            DataFormat::DataFrame(df) => Ok(df.clone()),
            DataFormat::RecordBatch(records) => {
                // Convert RecordBatch to DataFrame
                if records.is_empty() {
                    return Ok(DataFrame::empty());
                }

                // Build DataFrame from records
                let json_str = serde_json::to_string(records)?;
                let cursor = std::io::Cursor::new(json_str.as_bytes());
                let df = JsonReader::new(cursor).finish()?;
                Ok(df)
            }
            DataFormat::Raw(_bytes) => {
                anyhow::bail!("Cannot convert raw bytes to DataFrame without format information")
            }
            DataFormat::Stream(_) => {
                anyhow::bail!("Cannot synchronously convert stream to DataFrame. Use collect_stream() instead")
            }
        }
    }

    pub fn as_record_batch(&self) -> Result<RecordBatch> {
        match self {
            DataFormat::RecordBatch(records) => Ok(records.clone()),
            DataFormat::DataFrame(df) => {
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
                                    .and_then(|v| {
                                        serde_json::Number::from_f64(v).map(JsonValue::Number)
                                    })
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

                Ok(records)
            }
            DataFormat::Raw(_) => {
                anyhow::bail!("Cannot convert raw bytes to RecordBatch without format information")
            }
            DataFormat::Stream(_) => {
                anyhow::bail!("Cannot synchronously convert stream to RecordBatch. Use collect_stream() instead")
            }
        }
    }

    /// Check if this is a streaming data format
    pub fn is_stream(&self) -> bool {
        matches!(self, DataFormat::Stream(_))
    }

    /// Try to clone the data format (Stream cannot be cloned)
    pub fn try_clone(&self) -> Result<Self> {
        match self {
            DataFormat::DataFrame(df) => Ok(DataFormat::DataFrame(df.clone())),
            DataFormat::RecordBatch(batch) => Ok(DataFormat::RecordBatch(batch.clone())),
            DataFormat::Raw(data) => Ok(DataFormat::Raw(data.clone())),
            DataFormat::Stream(_) => {
                anyhow::bail!("Cannot clone streaming data. Streams can only be consumed once.")
            }
        }
    }
}

/// Streaming data source that produces a stream of data
#[async_trait]
pub trait StreamingDataSource: Send + Sync {
    async fn name(&self) -> &str;

    /// Create a stream of data
    async fn stream(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>>;

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()>;
}

pub type StreamingDataSourceRef = Arc<dyn StreamingDataSource>;
