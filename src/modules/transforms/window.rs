use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio_stream::StreamExt;
use tracing::info;

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::streaming::{StreamProcessor, StreamWindower, WindowType};
use crate::core::traits::DataFormat;

/// Window transform for streaming data
pub struct WindowTransform;

impl Default for WindowTransform {
    fn default() -> Self {
        Self::new()
    }
}

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
                    .ok_or_else(|| {
                        anyhow::anyhow!("Session window requires 'gap' parameter (seconds)")
                    })? as u64;

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
impl Stage for WindowTransform {
    fn name(&self) -> &str {
        "window"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert(
            "type".to_string(),
            toml::Value::String("tumbling".to_string()),
        );
        example1.insert("size".to_string(), toml::Value::Integer(100));

        let mut example2 = HashMap::new();
        example2.insert(
            "type".to_string(),
            toml::Value::String("sliding".to_string()),
        );
        example2.insert("size".to_string(), toml::Value::Integer(100));
        example2.insert("slide".to_string(), toml::Value::Integer(50));

        let mut example3 = HashMap::new();
        example3.insert(
            "type".to_string(),
            toml::Value::String("session".to_string()),
        );
        example3.insert("gap".to_string(), toml::Value::Integer(300));

        StageMetadata::builder("window", StageCategory::Transform)
            .description("Apply windowing to streaming data")
            .long_description(
                "Applies windowing strategies to streaming data for time-based or count-based grouping. \
                Supports three window types:\n\
                - Tumbling: Fixed-size, non-overlapping windows\n\
                - Sliding: Fixed-size, overlapping windows with configurable slide interval\n\
                - Session: Dynamic windows based on inactivity gaps\n\
                Enables temporal aggregations and stream processing patterns."
            )
            .parameter(ConfigParameter::optional(
                "type",
                ParameterType::String,
                "tumbling",
                "Window type"
            ).with_validation(ParameterValidation::allowed_values([
                "tumbling", "sliding", "session"
            ])))
            .parameter(ConfigParameter::optional(
                "size",
                ParameterType::Integer,
                "none",
                "Window size in number of records (required for tumbling and sliding)"
            ))
            .parameter(ConfigParameter::optional(
                "slide",
                ParameterType::Integer,
                "none",
                "Slide interval for sliding windows (required for sliding type)"
            ))
            .parameter(ConfigParameter::optional(
                "gap",
                ParameterType::Integer,
                "none",
                "Inactivity gap in seconds for session windows (required for session type)"
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Tumbling window",
                example1,
                Some("Group into fixed windows of 100 records")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Sliding window",
                example2,
                Some("Windows of 100 records, advancing 50 at a time")
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Session window",
                example3,
                Some("Dynamic windows with 5-minute inactivity gaps")
            ))
            .tag("window")
            .tag("stream")
            .tag("time-series")
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
            .ok_or_else(|| anyhow::anyhow!("Window transform requires input data"))?;

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
                let df =
                    crate::core::streaming::StreamBatcher::stream_to_dataframe(windowed).await?;
                Ok(DataFormat::DataFrame(df))
            }
            DataFormat::Raw(_) => {
                anyhow::bail!("Window transform does not support raw data format")
            }
        }
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
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
        config.insert(
            "type".to_string(),
            toml::Value::String("tumbling".to_string()),
        );
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
        config.insert(
            "type".to_string(),
            toml::Value::String("sliding".to_string()),
        );
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
        config.insert(
            "type".to_string(),
            toml::Value::String("tumbling".to_string()),
        );
        config.insert("size".to_string(), toml::Value::Integer(100));

        assert!(transform.validate_config(&config).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_missing_size() {
        let transform = WindowTransform::new();

        let mut config = HashMap::new();
        config.insert(
            "type".to_string(),
            toml::Value::String("tumbling".to_string()),
        );

        assert!(transform.validate_config(&config).await.is_err());
    }
}
