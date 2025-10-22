# Conveyor Kafka Plugin

Apache Kafka integration plugin for Conveyor ETL pipelines.

## Features

- **Consumer (Source)**: Read messages from Kafka topics
- **Producer (Sink)**: Write messages to Kafka topics
- **Message Metadata**: Automatic extraction of partition, offset, timestamp
- **Flexible Partitioning**: Support for key-based message partitioning
- **JSON Support**: Automatic JSON parsing and serialization

## Installation

### Build the Plugin

```bash
# Debug build
cargo build -p conveyor-plugin-kafka

# Release build
cargo build -p conveyor-plugin-kafka --release
```

The compiled plugin will be at:
- Debug: `target/debug/libconveyor_plugin_kafka.dylib` (macOS)
- Release: `target/release/libconveyor_plugin_kafka.dylib` (macOS)

### Enable in Pipeline

Add `kafka` to the plugins list in your pipeline configuration:

```toml
[global]
plugins = ["kafka"]
```

## Usage

### Consumer (Source)

Read messages from a Kafka topic:

```toml
[[stages]]
id = "consume"
function = "kafka"
inputs = []

[stages.config]
brokers = "localhost:9092"        # Required: Kafka brokers
topic = "my-topic"                # Required: Topic to consume from
group_id = "my-consumer-group"    # Required: Consumer group ID
max_messages = "1000"             # Optional: Max messages (default: 1000)
timeout_ms = "30000"              # Optional: Timeout (default: 30000)
```

**Configuration Options:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `brokers` | ✅ Yes | - | Comma-separated Kafka brokers (e.g., "localhost:9092") |
| `topic` | ✅ Yes | - | Kafka topic to consume from |
| `group_id` | ✅ Yes | - | Consumer group ID for offset management |
| `max_messages` | No | 1000 | Maximum messages to consume |
| `timeout_ms` | No | 30000 | Timeout in milliseconds |

**Message Format:**

Messages are automatically enriched with Kafka metadata:

```json
{
  "user_id": 123,
  "event": "purchase",
  "_kafka_key": "user-123",
  "_kafka_partition": 0,
  "_kafka_offset": 12345,
  "_kafka_timestamp": 1704067200000
}
```

For non-JSON messages, the raw payload is wrapped:

```json
{
  "_kafka_payload": "raw message content",
  "_kafka_key": "message-key",
  "_kafka_partition": 0,
  "_kafka_offset": 12345,
  "_kafka_timestamp": 1704067200000
}
```

### Producer (Sink)

Write messages to a Kafka topic:

```toml
[[stages]]
id = "produce"
function = "kafka"
inputs = ["previous_stage"]

[stages.config]
brokers = "localhost:9092"        # Required: Kafka brokers
topic = "output-topic"            # Required: Topic to produce to
key_field = "user_id"             # Optional: Field to use as message key
```

**Configuration Options:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `brokers` | ✅ Yes | - | Comma-separated Kafka brokers |
| `topic` | ✅ Yes | - | Kafka topic to produce to |
| `key_field` | No | - | Field name to use as message key (for partitioning) |

**Message Key:**

If `key_field` is specified, that field's value will be used as the Kafka message key for partitioning:

```toml
key_field = "user_id"  # Messages with same user_id go to same partition
```

## Examples

### Example 1: Simple Consumer

```toml
[pipeline]
name = "kafka-consumer"
version = "1.0.0"

[global]
plugins = ["kafka"]

[[stages]]
id = "consume"
function = "kafka"
inputs = []

[stages.config]
brokers = "localhost:9092"
topic = "events"
group_id = "conveyor-consumer"

[[stages]]
id = "save"
function = "json.write"
inputs = ["consume"]

[stages.config]
path = "output/events.json"
```

### Example 2: Simple Producer

```toml
[pipeline]
name = "kafka-producer"
version = "1.0.0"

[global]
plugins = ["kafka"]

[[stages]]
id = "load"
function = "json.read"
inputs = []

[stages.config]
path = "data/events.json"

[[stages]]
id = "produce"
function = "kafka"
inputs = ["load"]

[stages.config]
brokers = "localhost:9092"
topic = "output-events"
key_field = "event_id"
```

### Example 3: ETL Pipeline (Consumer → Transform → Producer)

```toml
[pipeline]
name = "kafka-etl"
version = "1.0.0"

[global]
plugins = ["kafka"]

# Consume from input topic
[[stages]]
id = "consume"
function = "kafka"
inputs = []

[stages.config]
brokers = "localhost:9092"
topic = "raw-events"
group_id = "etl-processor"

# Filter high-value events
[[stages]]
id = "filter"
function = "filter.apply"
inputs = ["consume"]

[stages.config]
column = "amount"
operator = ">="
value = "100"

# Produce to output topic
[[stages]]
id = "produce"
function = "kafka"
inputs = ["filter"]

[stages.config]
brokers = "localhost:9092"
topic = "processed-events"
key_field = "user_id"
```

## Testing

### Prerequisites

1. **Start Kafka** (using Docker Compose):

```bash
# docker-compose.yml
version: '3'
services:
  zookeeper:
    image: confluentinc/cp-zookeeper:latest
    environment:
      ZOOKEEPER_CLIENT_PORT: 2181
      ZOOKEEPER_TICK_TIME: 2000

  kafka:
    image: confluentinc/cp-kafka:latest
    depends_on:
      - zookeeper
    ports:
      - "9092:9092"
    environment:
      KAFKA_BROKER_ID: 1
      KAFKA_ZOOKEEPER_CONNECT: zookeeper:2181
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9092
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1
```

```bash
docker-compose up -d
```

2. **Create Test Topics**:

```bash
# Create input topic
docker exec -it kafka kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic test-topic \
  --partitions 3 \
  --replication-factor 1

# Create output topic
docker exec -it kafka kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic output-topic \
  --partitions 3 \
  --replication-factor 1
```

3. **Produce Test Messages**:

```bash
# Produce some test messages
docker exec -it kafka kafka-console-producer \
  --bootstrap-server localhost:9092 \
  --topic test-topic

# Then type JSON messages:
{"user_id": 1, "event": "login", "amount": 150}
{"user_id": 2, "event": "purchase", "amount": 200}
{"user_id": 3, "event": "logout", "amount": 50}
```

### Run Tests

```bash
# Run unit tests
cargo test -p conveyor-plugin-kafka

# Run example pipelines
cargo run --release -- run examples/kafka-consumer-example.toml
cargo run --release -- run examples/kafka-producer-example.toml
cargo run --release -- run examples/kafka-etl-example.toml
```

## Configuration Best Practices

### Consumer Configuration

1. **Consumer Groups**: Use meaningful group IDs for tracking and debugging
   ```toml
   group_id = "analytics-pipeline"  # Not "test" or "consumer1"
   ```

2. **Timeouts**: Set appropriate timeouts based on message volume
   ```toml
   timeout_ms = "60000"  # 1 minute for low-volume topics
   ```

3. **Batch Size**: Adjust max_messages based on memory and processing time
   ```toml
   max_messages = "10000"  # Higher for batch processing
   ```

### Producer Configuration

1. **Message Keys**: Use keys for guaranteed ordering per key
   ```toml
   key_field = "user_id"  # All messages for user go to same partition
   ```

2. **Error Handling**: Configure retry strategy
   ```toml
   [error_handling]
   strategy = "retry"
   max_retries = 3
   retry_delay_seconds = 5
   ```

## Performance Considerations

- **Consumer**: Messages are fetched in batches for efficiency
- **Producer**: Messages are sent sequentially (not batched)
- **Memory**: Large `max_messages` values may consume significant memory
- **Timeout**: Longer timeouts allow waiting for more messages

## Limitations

- **No Avro/Protobuf**: Currently supports JSON and text only
- **No SASL/SSL**: Authentication and encryption not yet implemented
- **Sequential Producer**: Producer sends messages one at a time
- **No Exactly-Once**: At-least-once delivery semantics only

## Dependencies

- `rdkafka` 0.36 - Rust Kafka client
- `tokio` - Async runtime
- `serde_json` - JSON serialization

## License

MIT

## See Also

- [Conveyor Documentation](../../README.md)
- [Plugin Development Guide](../../docs/plugin-system.md)
- [Apache Kafka Documentation](https://kafka.apache.org/documentation/)
