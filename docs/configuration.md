# Configuration Reference

Complete reference for Conveyor pipeline configuration.

## Pipeline Structure

```toml
[pipeline]
name = "pipeline_name"
version = "1.0.0"
description = "Pipeline description"

[global]
log_level = "info"
max_parallel_tasks = 4
timeout_seconds = 300
plugins = ["http", "mongodb"]

[[stages]]
id = "stage1"
type = "source.csv"
inputs = []
[stages.config]
# Stage-specific configuration

[error_handling]
strategy = "stop"
max_retries = 3
retry_delay_seconds = 5
```

## Pipeline Metadata

### [pipeline]

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | ✅ Yes | - | Pipeline name (unique identifier) |
| `version` | No | `"1.0.0"` | Semantic version |
| `description` | No | `""` | Human-readable description |

**Example:**

```toml
[pipeline]
name = "sales_etl"
version = "2.1.0"
description = "Daily sales data processing pipeline"
```

## Global Configuration

### [global]

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `log_level` | No | `"info"` | Logging level |
| `max_parallel_tasks` | No | `4` | Max concurrent tasks |
| `timeout_seconds` | No | `300` | Pipeline timeout (seconds) |
| `plugins` | No | `[]` | Plugins to load |

**Log Levels:**
- `trace`: Very detailed debug information
- `debug`: Debug information
- `info`: Informational messages (default)
- `warn`: Warning messages
- `error`: Error messages only

**Example:**

```toml
[global]
log_level = "debug"
max_parallel_tasks = 8
timeout_seconds = 600
plugins = ["http", "mongodb"]
```

## Stage Configuration

### [[stages]]

Each stage is defined with:

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `id` | ✅ Yes | - | Unique stage identifier |
| `type` | ✅ Yes | - | Stage type (category.name) |
| `inputs` | No | `[]` | List of input stage IDs |
| `config` | No | `{}` | Stage-specific configuration |

**Stage Types:**
- Built-in: `source.*`, `transform.*`, `sink.*`
- Plugins: `plugin.*`, `wasm.*`
- Special: `stage.pipeline`

**Example:**

```toml
[[stages]]
id = "load_data"
type = "source.csv"
inputs = []

[stages.config]
path = "data/input.csv"
headers = true
```

### Stage Dependencies

Use the `inputs` field to define dependencies:

```toml
# Stage 1: No dependencies
[[stages]]
id = "source"
type = "source.json"
inputs = []

# Stage 2: Depends on source
[[stages]]
id = "transform"
type = "transform.filter"
inputs = ["source"]

# Stage 3: Depends on transform
[[stages]]
id = "sink"
type = "sink.csv"
inputs = ["transform"]
```

### Multiple Inputs

Stages can have multiple inputs:

```toml
[[stages]]
id = "users"
type = "source.json"
inputs = []

[[stages]]
id = "orders"
type = "source.csv"
inputs = []

[[stages]]
id = "join"
type = "transform.join"
inputs = ["users", "orders"]  # Multiple inputs
```

## Error Handling

### [error_handling]

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `strategy` | No | `"stop"` | Error handling strategy |
| `max_retries` | No | `0` | Max retry attempts |
| `retry_delay_seconds` | No | `0` | Delay between retries |
| `dead_letter_queue` | No | - | Failed record handling |

**Strategies:**
- `stop`: Stop pipeline on first error
- `continue`: Skip failed stage, continue with empty data
- `retry`: Retry failed stage up to `max_retries` times

**Example:**

```toml
[error_handling]
strategy = "retry"
max_retries = 3
retry_delay_seconds = 5

[error_handling.dead_letter_queue]
enabled = true
path = "errors/"
```

### Dead Letter Queue

Save failed records to a file:

```toml
[error_handling.dead_letter_queue]
enabled = true
path = "errors/failed_records.json"
```

Failed records are saved with error metadata:

```json
{
  "record": {...},
  "error": "Error message",
  "stage_id": "stage_name",
  "timestamp": "2024-01-01T12:00:00Z"
}
```

## Stage-Specific Configuration

### Source Stages

See [Modules Reference](modules-reference.md) for complete options.

**CSV:**
```toml
[stages.config]
path = "data.csv"
headers = true
delimiter = ","
```

**JSON:**
```toml
[stages.config]
path = "data.json"
format = "records"  # records, jsonl, dataframe
```

**HTTP Plugin:**
```toml
[stages.config]
url = "https://api.example.com/data"
method = "GET"
format = "json"
timeout = 30

[stages.config.headers]
Authorization = "Bearer TOKEN"
```

### Transform Stages

**Filter:**
```toml
[stages.config]
column = "amount"
operator = ">="
value = 100
```

**Map:**
```toml
[stages.config]
expression = "price * quantity"
output_column = "total"
```

**Validate Schema:**
```toml
[stages.config]
required_fields = ["id", "name"]
non_nullable = ["id"]

[stages.config.field_types]
id = "int"
name = "string"
```

**HTTP Fetch:**
```toml
[stages.config]
url = "https://api.example.com/users/{{ id }}"
method = "GET"
result_field = "user_data"
mode = "per_row"  # or "batch"
```

### Sink Stages

**CSV:**
```toml
[stages.config]
path = "output.csv"
headers = true
delimiter = ","
```

**JSON:**
```toml
[stages.config]
path = "output.json"
format = "records"  # records, jsonl
pretty = true
```

**Stdout:**
```toml
[stages.config]
format = "table"  # table, json, jsonl, csv
limit = 10
```

## Environment Variables

Use environment variables in configuration:

```toml
[stages.config]
connection_string = "{{ env.MONGODB_URI }}"

[stages.config.headers]
Authorization = "Bearer {{ env.API_TOKEN }}"
```

**Usage:**
```bash
export API_TOKEN="sk_live_..."
export MONGODB_URI="mongodb://localhost:27017"
conveyor run -c pipeline.toml
```

## TOML Tips

### Multi-line Strings

Use triple quotes for multi-line values:

```toml
[stages.config]
body = '''
{
  "name": "{{ name }}",
  "items": [
    {{#each items}}
    "{{ this }}"{{#unless @last}},{{/unless}}
    {{/each}}
  ]
}
'''
```

### Arrays

```toml
[stages.config]
required_fields = ["id", "name", "email"]
```

### Tables

```toml
[stages.config.headers]
Content-Type = "application/json"
Authorization = "Bearer TOKEN"
X-Custom-Header = "value"
```

### Nested Tables

```toml
[stages.config]
field1 = "value1"

[stages.config.nested]
field2 = "value2"
field3 = "value3"
```

## Validation

Conveyor validates configuration before execution:

### Stage ID Validation

- Must be unique
- Alphanumeric + underscores
- Cannot be empty

### Input Validation

- All input stage IDs must exist
- No cycles (DAG requirement)
- At least one stage with no inputs (source)

### Type Validation

- Stage type must be valid format: `category.name`
- Category must be: `source`, `transform`, `sink`, `plugin`, `wasm`, `stage`

### Config Validation

Each stage type validates its configuration:

```toml
# This will fail validation
[[stages]]
id = "csv_source"
type = "source.csv"
# Missing required 'path' in config!
```

## Complete Example

```toml
[pipeline]
name = "user_analytics"
version = "1.0.0"
description = "Process user events and generate analytics"

[global]
log_level = "info"
max_parallel_tasks = 4
timeout_seconds = 600
plugins = ["http", "mongodb"]

# Stage 1: Load events from MongoDB
[[stages]]
id = "load_events"
type = "plugin.mongodb"
inputs = []

[stages.config]
connection_string = "{{ env.MONGODB_URI }}"
database = "analytics"
collection = "events"
query = '{ "event_type": "purchase" }'
batch_size = 5000

# Stage 2: Validate data
[[stages]]
id = "validate"
type = "transform.validate_schema"
inputs = ["load_events"]

[stages.config]
required_fields = ["user_id", "event_type", "amount"]
non_nullable = ["user_id"]

[stages.config.field_types]
user_id = "string"
amount = "float"

# Stage 3: Filter high-value events
[[stages]]
id = "filter_high_value"
type = "transform.filter"
inputs = ["validate"]

[stages.config]
column = "amount"
operator = ">="
value = 100.0

# Stage 4: Enrich with user data via API
[[stages]]
id = "enrich_users"
type = "transform.http_fetch"
inputs = ["filter_high_value"]

[stages.config]
url = "https://api.example.com/users/{{ user_id }}"
result_field = "user_profile"

[stages.config.headers]
Authorization = "Bearer {{ env.API_TOKEN }}"

# Stage 5: Save to JSON (branch 1)
[[stages]]
id = "save_json"
type = "sink.json"
inputs = ["enrich_users"]

[stages.config]
path = "output/enriched_events.json"
format = "jsonl"

# Stage 6: Display preview (branch 2)
[[stages]]
id = "preview"
type = "sink.stdout"
inputs = ["enrich_users"]

[stages.config]
format = "table"
limit = 10

[error_handling]
strategy = "continue"
max_retries = 3
retry_delay_seconds = 5

[error_handling.dead_letter_queue]
enabled = true
path = "errors/failed_records.jsonl"
```

## See Also

- [DAG Pipelines](dag-pipelines.md) - Pipeline composition
- [Modules Reference](modules-reference.md) - All available modules
- [Plugin System](plugin-system.md) - Plugin configuration
