//! Kafka Plugin for Conveyor
//!
//! Provides Kafka consumer (source) and producer (sink) functionality.
//! Uses rdkafka for Apache Kafka integration.

use conveyor_plugin_api::sabi_trait::prelude::*;
use conveyor_plugin_api::traits::{FfiExecutionContext, FfiStage, FfiStage_TO};
use conveyor_plugin_api::{
    rstr, FfiDataFormat, PluginCapability, PluginDeclaration, RBox, RBoxError, RErr, RHashMap, ROk,
    RResult, RString, RVec, StageType, PLUGIN_API_VERSION,
};
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    producer::{FutureProducer, FutureRecord, Producer},
    ClientConfig, Message,
};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

/// Kafka Stage - unified consumer and producer
pub struct KafkaStage {
    name: String,
    stage_type: StageType,
}

impl KafkaStage {
    fn new(name: String, stage_type: StageType) -> Self {
        Self { name, stage_type }
    }

    /// Execute as Kafka consumer (source)
    async fn execute_source_async(
        &self,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let brokers = match config.get("brokers") {
            Some(b) => b,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'brokers' configuration"
                )))
            }
        };

        let topic = match config.get("topic") {
            Some(t) => t,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'topic' configuration"
                )))
            }
        };

        let group_id = match config.get("group_id") {
            Some(g) => g,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'group_id' configuration"
                )))
            }
        };

        let max_messages: usize = config
            .get("max_messages")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        let timeout_ms: u64 = config
            .get("timeout_ms")
            .and_then(|s| s.parse().ok())
            .unwrap_or(30000);

        // Create consumer
        let consumer: StreamConsumer = match ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create()
        {
            Ok(c) => c,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create Kafka consumer: {}",
                    e
                )))
            }
        };

        // Subscribe to topic
        if let Err(e) = consumer.subscribe(&[topic]) {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Failed to subscribe to topic: {}",
                e
            )));
        }

        // Collect messages
        let mut records = Vec::new();
        let start = std::time::Instant::now();
        let timeout_duration = Duration::from_millis(timeout_ms);

        while records.len() < max_messages {
            // Check timeout
            if start.elapsed() >= timeout_duration {
                break;
            }

            // Try to receive message with timeout
            let remaining = timeout_duration - start.elapsed();
            match timeout(remaining, consumer.recv()).await {
                Ok(Ok(message)) => {
                    // Extract message data
                    let payload = message.payload().unwrap_or(&[]);
                    let key = message
                        .key()
                        .map(|k| String::from_utf8_lossy(k).to_string());

                    // Try to parse as JSON
                    match serde_json::from_slice::<Value>(payload) {
                        Ok(mut json_value) => {
                            // Add metadata if JSON object
                            if let Value::Object(ref mut map) = json_value {
                                if let Some(k) = key {
                                    map.insert("_kafka_key".to_string(), Value::String(k));
                                }
                                map.insert(
                                    "_kafka_partition".to_string(),
                                    Value::Number(message.partition().into()),
                                );
                                map.insert(
                                    "_kafka_offset".to_string(),
                                    Value::Number(message.offset().into()),
                                );
                                if let Some(timestamp) = message.timestamp().to_millis() {
                                    map.insert(
                                        "_kafka_timestamp".to_string(),
                                        Value::Number(timestamp.into()),
                                    );
                                }
                            }
                            records.push(json_value);
                        }
                        Err(_) => {
                            // If not JSON, create a wrapper object
                            let mut wrapper = serde_json::Map::new();
                            wrapper.insert(
                                "_kafka_payload".to_string(),
                                Value::String(String::from_utf8_lossy(payload).to_string()),
                            );
                            if let Some(k) = key {
                                wrapper.insert("_kafka_key".to_string(), Value::String(k));
                            }
                            wrapper.insert(
                                "_kafka_partition".to_string(),
                                Value::Number(message.partition().into()),
                            );
                            wrapper.insert(
                                "_kafka_offset".to_string(),
                                Value::Number(message.offset().into()),
                            );
                            if let Some(timestamp) = message.timestamp().to_millis() {
                                wrapper.insert(
                                    "_kafka_timestamp".to_string(),
                                    Value::Number(timestamp.into()),
                                );
                            }
                            records.push(Value::Object(wrapper));
                        }
                    }
                }
                Ok(Err(e)) => {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Error receiving message: {}",
                        e
                    )));
                }
                Err(_) => {
                    // Timeout on recv - no more messages
                    break;
                }
            }
        }

        // Convert to FfiDataFormat
        match FfiDataFormat::from_json_records(&records) {
            ROk(data) => ROk(data),
            RErr(e) => RErr(e),
        }
    }

    /// Execute as Kafka producer (sink)
    async fn execute_sink_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let brokers = match config.get("brokers") {
            Some(b) => b,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'brokers' configuration"
                )))
            }
        };

        let topic = match config.get("topic") {
            Some(t) => t,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'topic' configuration"
                )))
            }
        };

        let key_field = config.get("key_field").map(|s| s.as_str());

        // Create producer
        let producer: FutureProducer = match ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "30000")
            .create()
        {
            Ok(p) => p,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create Kafka producer: {}",
                    e
                )))
            }
        };

        // Convert data to records
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        // Send messages
        for record in records.iter() {
            // Extract key if specified
            let key = if let Some(key_field_name) = key_field {
                record
                    .get(key_field_name)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            // Serialize record to JSON
            let payload = match serde_json::to_string(&record) {
                Ok(s) => s,
                Err(e) => {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Failed to serialize record: {}",
                        e
                    )))
                }
            };

            // Create Kafka record
            let mut kafka_record = FutureRecord::to(topic).payload(&payload);

            if let Some(k) = &key {
                kafka_record = kafka_record.key(k);
            }

            // Send and wait
            match producer.send(kafka_record, Duration::from_secs(30)).await {
                Ok(_) => {}
                Err((err, _)) => {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Failed to send message: {}",
                        err
                    )));
                }
            }
        }

        // Flush producer
        if let Err(e) = producer.flush(Duration::from_secs(10)) {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Failed to flush producer: {}",
                e
            )));
        }

        // Return the original data (sinks pass through data)
        ROk(input_data.clone())
    }
}

impl FfiStage for KafkaStage {
    fn name(&self) -> conveyor_plugin_api::RStr<'_> {
        self.name.as_str().into()
    }

    fn stage_type(&self) -> StageType {
        self.stage_type
    }

    fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
        // Convert config to HashMap
        let config: HashMap<String, String> = context
            .config
            .into_iter()
            .map(|tuple| (tuple.0.to_string(), tuple.1.to_string()))
            .collect();

        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create runtime: {}",
                    e
                )))
            }
        };

        runtime.block_on(async {
            match self.stage_type {
                StageType::Source => {
                    // Source mode: consume from Kafka
                    self.execute_source_async(&config).await
                }
                StageType::Sink => {
                    // Sink mode: produce to Kafka
                    // Get input data (should have exactly one input)
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "Kafka sink requires input data"
                            )))
                        }
                    };

                    self.execute_sink_async(&input_data, &config).await
                }
                StageType::Transform => RErr(RBoxError::from_fmt(&format_args!(
                    "Kafka transform not supported"
                ))),
            }
        })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Check required fields
        if !config.contains_key("brokers") {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Missing required 'brokers' configuration"
            )));
        }

        if !config.contains_key("topic") {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Missing required 'topic' configuration"
            )));
        }

        // Source requires group_id
        if self.stage_type == StageType::Source && !config.contains_key("group_id") {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Kafka source requires 'group_id' configuration"
            )));
        }

        // Validate max_messages if provided
        if let Some(max_messages) = config.get("max_messages") {
            if max_messages.parse::<usize>().is_err() {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "'max_messages' must be a positive integer"
                )));
            }
        }

        // Validate timeout_ms if provided
        if let Some(timeout_ms) = config.get("timeout_ms") {
            if timeout_ms.parse::<u64>().is_err() {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "'timeout_ms' must be a positive integer"
                )));
            }
        }

        ROk(())
    }
}

// Factory functions
#[no_mangle]
pub extern "C" fn create_kafka_source() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        KafkaStage::new("kafka".to_string(), StageType::Source),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_kafka_sink() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        KafkaStage::new("kafka".to_string(), StageType::Sink),
        TD_Opaque,
    )
}

// Plugin capabilities
extern "C" fn get_capabilities() -> RVec<PluginCapability> {
    vec![
        PluginCapability::simple(
            "kafka",
            StageType::Source,
            "Kafka consumer - consume messages from Kafka topics",
            "create_kafka_source",
        ),
        PluginCapability::simple(
            "kafka",
            StageType::Sink,
            "Kafka producer - produce messages to Kafka topics",
            "create_kafka_sink",
        ),
    ]
    .into()
}

// Plugin declaration
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("kafka"),
    version: rstr!("0.1.0"),
    description: rstr!("Kafka consumer and producer plugin for Apache Kafka integration"),
    get_capabilities,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kafka_source() {
        let stage = KafkaStage::new("kafka".to_string(), StageType::Source);
        assert_eq!(stage.name(), "kafka");
        assert_eq!(stage.stage_type(), StageType::Source);
    }

    #[test]
    fn test_kafka_sink() {
        let stage = KafkaStage::new("kafka".to_string(), StageType::Sink);
        assert_eq!(stage.name(), "kafka");
        assert_eq!(stage.stage_type(), StageType::Sink);
    }

    #[test]
    fn test_plugin_declaration() {
        assert_eq!(_plugin_declaration.name, "kafka");
        assert_eq!(_plugin_declaration.version, "0.1.0");
        assert!(_plugin_declaration.is_compatible());
    }

    #[test]
    fn test_source_validation() {
        let stage = KafkaStage::new("kafka".to_string(), StageType::Source);
        let mut config = RHashMap::new();

        // Missing brokers should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // Add brokers
        config.insert(RString::from("brokers"), RString::from("localhost:9092"));

        // Missing topic should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // Add topic
        config.insert(RString::from("topic"), RString::from("test-topic"));

        // Missing group_id should fail for source
        assert!(stage.validate_config(config.clone()).is_err());

        // Add group_id
        config.insert(
            RString::from("group_id"),
            RString::from("test-consumer-group"),
        );

        // Should succeed
        assert!(stage.validate_config(config).is_ok());
    }

    #[test]
    fn test_sink_validation() {
        let stage = KafkaStage::new("kafka".to_string(), StageType::Sink);
        let mut config = RHashMap::new();

        // Missing brokers should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // Add brokers
        config.insert(RString::from("brokers"), RString::from("localhost:9092"));

        // Missing topic should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // Add topic
        config.insert(RString::from("topic"), RString::from("test-topic"));

        // Should succeed (no group_id required for sink)
        assert!(stage.validate_config(config).is_ok());
    }

    #[test]
    fn test_capabilities() {
        let caps = get_capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].name.as_str(), "kafka");
        assert_eq!(caps[0].stage_type, StageType::Source);
        assert_eq!(caps[1].stage_type, StageType::Sink);
    }
}
