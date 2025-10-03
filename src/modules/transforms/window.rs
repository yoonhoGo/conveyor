use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio_stream::StreamExt;
use tracing::info;

use crate::core::streaming::{StreamProcessor, StreamWindower, WindowType};
use crate::core::traits::{DataFormat, Transform};

/// Window transform for streaming data
pub struct WindowTransform;

impl WindowTransform {
    pub fn new() -> Self {
        Self
    }

    /// Parse window configuration
    fn parse_window_config(config: &HashMap<String, toml::Value>) -> Result<WindowType> {
        let window_type = config
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("tumbling");

        match window_type {
            "tumbling" => {
                let size = config
                    .get("size")
                    .and_then(|v| v.as_integer())
                    .ok_or_else(|| anyhow::anyhow!("Tumbling window requires 'size' parameter"))?
                    as usize;

                Ok(WindowType::Tumbling { size })
            }
            "sliding" => {
                let size = config
                    .get("size")
                    .and_then(|v| v.as_integer())
                    .ok_or_else(|| anyhow::anyhow!("Sliding window requires 'size' parameter"))?
                    as usize;

                let slide = config
                    .get("slide")
                    .and_then(|v| v.as_integer())
                    .ok_or_else(|| anyhow::anyhow!("Sliding window requires 'slide' parameter"))?
                    as usize;

                Ok(WindowType::Sliding { size, slide })
            }
            "session" => {
                let gap_secs = config
                    .get("gap")
                    .and_then(|v| v.as_integer())
                    .ok_or_else(|| anyhow::anyhow!("Session window requires 'gap' parameter (seconds)"))?
                    as u64;

                Ok(WindowType::Session {
                    gap: std::time::Duration::from_secs(gap_secs),
                })
            }
            _ => {
                anyhow::bail!(
                    "Invalid window type: {}. Must be 'tumbling', 'sliding', or 'session'",
                    window_type
                )
            }
        }
    }
}

#[async_trait]
impl Transform for WindowTransform {
    async fn name(&self) -> &str {
        "window"
    }

    async fn apply(
        &self,
        data: DataFormat,
        config: &Option<HashMap<String, toml::Value>>,
    ) -> Result<DataFormat> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Window transform requires configuration"))?;

        let window_type = Self::parse_window_config(config)?;

        info!("Applying windowing: {:?}", window_type);

        match data {
            DataFormat::Stream(stream) => {
                let windower = StreamWindower::new(window_type);
                let windowed = windower.apply(stream);
                Ok(DataFormat::Stream(windowed))
            }
            DataFormat::RecordBatch(batch) => {
                // For batch data, convert to stream, apply window, collect back
                let stream = StreamProcessor::from_batch(batch);
                let windower = StreamWindower::new(window_type);
                let windowed = windower.apply(stream);

                // Collect the windowed stream
                let batches: Vec<_> = windowed.collect().await;
                let combined: Result<Vec<_>> = batches.into_iter().collect();
                let records: Vec<_> = combined?.into_iter().flatten().collect();

                Ok(DataFormat::RecordBatch(records))
            }
            DataFormat::DataFrame(df) => {
                // For DataFrame, convert to stream, apply window, collect back
                let stream = StreamProcessor::from_dataframe(df)?;
                let windower = StreamWindower::new(window_type);
                let windowed = windower.apply(stream);

                // Collect back to DataFrame
                let df = crate::core::streaming::StreamBatcher::stream_to_dataframe(windowed).await?;
                Ok(DataFormat::DataFrame(df))
            }
            DataFormat::Raw(_) => {
                anyhow::bail!("Window transform does not support raw data format")
            }
        }
    }

    async fn validate_config(&self, config: &Option<HashMap<String, toml::Value>>) -> Result<()> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Window transform requires configuration"))?;

        // Try to parse the window config
        Self::parse_window_config(config)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_window_config_tumbling() {
        let mut config = HashMap::new();
        config.insert("type".to_string(), toml::Value::String("tumbling".to_string()));
        config.insert("size".to_string(), toml::Value::Integer(100));

        let window_type = WindowTransform::parse_window_config(&config).unwrap();

        match window_type {
            WindowType::Tumbling { size } => assert_eq!(size, 100),
            _ => panic!("Expected tumbling window"),
        }
    }

    #[test]
    fn test_parse_window_config_sliding() {
        let mut config = HashMap::new();
        config.insert("type".to_string(), toml::Value::String("sliding".to_string()));
        config.insert("size".to_string(), toml::Value::Integer(100));
        config.insert("slide".to_string(), toml::Value::Integer(50));

        let window_type = WindowTransform::parse_window_config(&config).unwrap();

        match window_type {
            WindowType::Sliding { size, slide } => {
                assert_eq!(size, 100);
                assert_eq!(slide, 50);
            }
            _ => panic!("Expected sliding window"),
        }
    }

    #[tokio::test]
    async fn test_validate_config() {
        let transform = WindowTransform::new();

        let mut config = HashMap::new();
        config.insert("type".to_string(), toml::Value::String("tumbling".to_string()));
        config.insert("size".to_string(), toml::Value::Integer(100));

        assert!(transform.validate_config(&Some(config)).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_missing_size() {
        let transform = WindowTransform::new();

        let mut config = HashMap::new();
        config.insert("type".to_string(), toml::Value::String("tumbling".to_string()));

        assert!(transform.validate_config(&Some(config)).await.is_err());
    }
}
