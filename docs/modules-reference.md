# Modules Reference

Complete reference for all built-in and plugin modules in Conveyor.

## Table of Contents

- [Global Configuration](#global-configuration)
- [Data Sources](#data-sources)
- [Transforms](#transforms)
- [Sinks](#sinks)
- [Plugin Modules](#plugin-modules)

## Global Configuration

### Variables System

Define reusable variables with environment variable substitution support.

**Configuration:**

```toml
[global.variables]
# Environment variable substitution: ${ENV_VAR}
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.example.com"

# Mix literal and environment variables
auth_token = "Bearer ${API_TOKEN}"
```

**Variable Interpolation:**

Use `{{var_name}}` in stage configurations to reference global variables.

```toml
[[stages]]
id = "stage1"
function = "http.get"

[stages.config]
url = "{{base_url}}/endpoint"
headers = { "Authorization" = "{{auth_token}}" }
```

**Features:**
- Environment variable substitution with `${ENV_VAR}` syntax
- Variable interpolation with `{{var_name}}` syntax
- Automatic resolution during pipeline loading
- Clear error messages for missing variables

---

## Data Sources

### csv.read

Read data from CSV files using Polars.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `path` | String | ✅ Yes | - | Path to CSV file |
| `has_headers` | Boolean | No | `true` | First row contains headers |
| `delimiter` | String | No | `,` | Column delimiter character |

**Example:**

```toml
[[stages]]
id = "load_csv"
function = "csv.read"
inputs = []

[stages.config]
path = "data/sales.csv"
has_headers = true
delimiter = ","
```

---

### json.read

Read data from JSON files.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `path` | String | ✅ Yes | - | Path to JSON file |
| `format` | String | No | `records` | Format: `records`, `jsonl`, `dataframe` |

**Formats:**
- `records`: JSON array of objects `[{...}, {...}]`
- `jsonl`: Newline-delimited JSON (one object per line)
- `dataframe`: Polars DataFrame JSON format

**Example:**

```toml
[[stages]]
id = "load_json"
function = "json.read"
inputs = []

[stages.config]
path = "data/users.json"
format = "records"
```

---

### stdin.read

Read data from standard input (batch mode).

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `format` | String | No | `json` | Format: `json`, `jsonl`, `csv`, `raw` |

**Example:**

```toml
[[stages]]
id = "read_stdin"
function = "stdin.read"
inputs = []

[stages.config]
format = "jsonl"
```

**Usage:**
```bash
cat data.json | conveyor run -c pipeline.toml
```

---

### stdin.stream

Read data from standard input in streaming mode (line-by-line).

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `format` | String | No | `jsonl` | Format: `jsonl`, `json` |

**Example:**

```toml
[[stages]]
id = "stream_input"
function = "stdin.stream"
inputs = []

[stages.config]
format = "jsonl"
```

---

### file.watch

Monitor a file for changes using polling.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `path` | String | ✅ Yes | - | Path to file to monitor |
| `format` | String | No | `jsonl` | Format: `jsonl`, `json`, `csv` |
| `poll_interval_ms` | Integer | No | `1000` | Polling interval in milliseconds |

**Example:**

```toml
[[stages]]
id = "watch_logs"
function = "file.watch"
inputs = []

[stages.config]
path = "/var/log/app.log"
format = "jsonl"
poll_interval_ms = 500
```

---

## Transforms

### filter.apply

Filter rows based on conditions.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `column` | String | ✅ Yes | - | Column name to filter on |
| `operator` | String | ✅ Yes | - | Comparison operator |
| `value` | Any | ✅ Yes | - | Value to compare against |

**Operators:**
- `==`: Equal
- `!=`: Not equal
- `>`: Greater than
- `>=`: Greater than or equal
- `<`: Less than
- `<=`: Less than or equal
- `contains`: String contains (case-sensitive)
- `in`: Value in list

**Example:**

```toml
[[stages]]
id = "filter_high_value"
function = "filter.apply"
inputs = ["data"]

[stages.config]
column = "amount"
operator = ">="
value = 1000.0
```

---

### map.apply

Create or transform columns using simple expressions.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `expression` | String | ✅ Yes | - | Mathematical expression |
| `output_column` | String | ✅ Yes | - | Name of the output column |

**Supported Operators:**
- `+`, `-`, `*`, `/`: Arithmetic (one operator per expression)
- Column references by name
- Column-to-column division

**Examples:**

```toml
# Simple arithmetic
[[stages]]
id = "add_tax"
function = "map.apply"
inputs = ["sales"]

[stages.config]
expression = "price * 1.1"
output_column = "price_with_tax"
```

```toml
# Column division
[[stages]]
id = "calc_unit_price"
function = "map.apply"
inputs = ["data"]

[stages.config]
expression = "total_price / quantity"
output_column = "unit_price"
```

---

### select.apply

Select specific columns from the data.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `columns` | String or Array | ✅ Yes | - | Column name(s) to select |

**Example:**

```toml
[[stages]]
id = "select_fields"
function = "select.apply"
inputs = ["data"]

[stages.config]
columns = ["name", "email", "age"]
```

---

### groupby.apply

Group data and perform aggregations.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `by` | String or Array | ✅ Yes | - | Column(s) to group by |
| `aggregations` | Array | ✅ Yes | - | List of aggregation operations |

**Aggregation Operations:**
- `sum`, `avg`, `mean`, `count`, `min`, `max`
- `median`, `std`, `var`, `first`, `last`

**Each aggregation requires:**
- `column`: Column to aggregate
- `operation`: Aggregation operation
- `output_column` (optional): Name for output column

**Example:**

```toml
[[stages]]
id = "sales_by_region"
function = "groupby.apply"
inputs = ["sales"]

[stages.config]
by = ["region", "category"]

[[stages.config.aggregations]]
column = "revenue"
operation = "sum"
output_column = "total_revenue"

[[stages.config.aggregations]]
column = "revenue"
operation = "avg"
output_column = "avg_revenue"

[[stages.config.aggregations]]
column = "order_id"
operation = "count"
output_column = "num_orders"
```

---

### sort.apply

Sort data by one or more columns.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `by` | String or Array | ✅ Yes | - | Column(s) to sort by |
| `descending` | Boolean or Array | No | `false` | Sort order (per column if array) |

**Example:**

```toml
# Single column sort
[[stages]]
id = "sort_by_date"
function = "sort.apply"
inputs = ["data"]

[stages.config]
by = "created_at"
descending = true
```

```toml
# Multi-column sort
[[stages]]
id = "sort_multi"
function = "sort.apply"
inputs = ["data"]

[stages.config]
by = ["category", "price"]
descending = [false, true]  # category ascending, price descending
```

---

### distinct.apply

Remove duplicate rows based on specified columns.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `subset` | Array | No | All columns | Columns to consider for uniqueness |

**Example:**

```toml
# Remove duplicates based on all columns
[[stages]]
id = "remove_dupes"
function = "distinct.apply"
inputs = ["data"]
```

```toml
# Remove duplicates based on specific columns
[[stages]]
id = "unique_users"
function = "distinct.apply"
inputs = ["users"]

[stages.config]
subset = ["email"]
```

---

### json.extract

Extract nested fields from JSON strings.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `column` | String | ✅ Yes | - | Column containing JSON strings |
| `path` | String | ✅ Yes | - | Dot-notation path to field |
| `output_column` | String | ✅ Yes | - | Name for output column |

**Example:**

```toml
[[stages]]
id = "extract_trace_id"
function = "json.extract"
inputs = ["logs"]

[stages.config]
column = "payload"
path = "meta.req.headers.x-trace-id"
output_column = "trace_id"
```

**Features:**
- Supports deeply nested JSON paths
- Handles missing fields gracefully (returns null)
- Supports strings, numbers, booleans, and complex types

---

### ai.generate

Generate content using LLM APIs (OpenAI, Anthropic, OpenRouter, Ollama).

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `provider` | String | ✅ Yes | - | `openai`, `anthropic`, `openrouter`, `ollama` |
| `model` | String | ✅ Yes | - | Model name (e.g., `gpt-4`, `claude-3-5-sonnet-20241022`) |
| `prompt` | String | ✅ Yes | - | Prompt template with `{{column}}` placeholders |
| `output_column` | String | ✅ Yes | - | Name for output column |
| `api_key_env` | String | No | Auto-detected | Environment variable for API key |
| `max_tokens` | Integer | No | - | Maximum tokens to generate |
| `temperature` | Float | No | - | Sampling temperature (0.0-1.0) |
| `api_base_url` | String | No | `http://localhost:11434` | Base URL (Ollama only) |

**Default API Key Environment Variables:**
- OpenAI: `OPENAI_API_KEY`
- Anthropic: `ANTHROPIC_API_KEY`
- OpenRouter: `OPENROUTER_API_KEY`
- Ollama: No API key required

**Examples:**

```toml
# OpenAI
[[stages]]
id = "summarize"
function = "ai.generate"
inputs = ["articles"]

[stages.config]
provider = "openai"
model = "gpt-4"
prompt = "Summarize this article in one sentence: {{content}}"
output_column = "summary"
max_tokens = 100
temperature = 0.7
```

```toml
# Anthropic
[[stages]]
id = "classify"
function = "ai.generate"
inputs = ["reviews"]

[stages.config]
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
prompt = "Classify sentiment (positive/negative/neutral): {{text}}"
output_column = "sentiment"
max_tokens = 10
```

```toml
# Ollama (local)
[[stages]]
id = "extract_entities"
function = "ai.generate"
inputs = ["documents"]

[stages.config]
provider = "ollama"
model = "llama2"
prompt = "Extract named entities from: {{text}}"
output_column = "entities"
api_base_url = "http://localhost:11434"
```

**Features:**
- Row-by-row processing with template prompts
- Handlebars syntax for referencing columns
- Support for multiple providers
- Configurable parameters per request

---

### validate.schema

Validate data schema and types.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `required_fields` | Array | No | `[]` | Required field names |
| `field_types` | Object | No | `{}` | Field name → type mapping |
| `non_nullable` | Array | No | `[]` | Fields that cannot be null |
| `unique` | Array | No | `[]` | Fields that must be unique |

**Field Types:**
- `string`, `int`, `float`, `bool`, `date`

**Example:**

```toml
[[stages]]
id = "validate"
function = "validate.schema"
inputs = ["data"]

[stages.config]
required_fields = ["id", "name", "email"]
non_nullable = ["id", "email"]
unique = ["email"]

[stages.config.field_types]
id = "int"
name = "string"
email = "string"
age = "int"
```

---

### http.fetch

Make HTTP requests using row data with template URLs.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `url` | String | ✅ Yes | - | URL template with `{{ column }}` placeholders |
| `method` | String | No | `GET` | HTTP method |
| `result_field` | String | No | - | Field name to store response |
| `headers` | Object | No | `{}` | Custom HTTP headers |
| `timeout_seconds` | Integer | No | `30` | Request timeout |

**Example:**

```toml
[[stages]]
id = "fetch_profiles"
function = "http.fetch"
inputs = ["users"]

[stages.config]
url = "https://api.example.com/users/{{ id }}/profile"
method = "GET"
result_field = "profile"
timeout_seconds = 10

[stages.config.headers]
Authorization = "Bearer ${API_TOKEN}"
```

See [HTTP Fetch Transform](http-fetch-transform.md) for detailed documentation.

---

### reduce.apply

Reduce data to a single aggregated row.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `column` | String | ✅ Yes | - | Column to reduce |
| `operation` | String | ✅ Yes | - | Reduction operation |
| `output_column` | String | No | Same as column | Output column name |

**Operations:**
- `sum`, `avg`, `min`, `max`, `count`

**Example:**

```toml
[[stages]]
id = "total_revenue"
function = "reduce.apply"
inputs = ["sales"]

[stages.config]
column = "revenue"
operation = "sum"
output_column = "total_revenue"
```

---

### window.apply

Apply windowing for stream processing.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `type` | String | ✅ Yes | - | Window type: `tumbling`, `sliding`, `session` |
| `size` | Integer | ✅ Yes | - | Window size (records or seconds) |
| `slide` | Integer | No | - | Slide size (sliding windows only) |
| `gap` | Integer | No | `5` | Session gap in seconds |

**Window Types:**
- `tumbling`: Non-overlapping fixed-size windows
- `sliding`: Overlapping windows with slide interval
- `session`: Dynamic windows based on inactivity gap

**Example:**

```toml
# Tumbling window
[[stages]]
id = "window"
function = "window.apply"
inputs = ["stream"]

[stages.config]
type = "tumbling"
size = 100  # 100 records per window
```

---

### aggregate.stream

Real-time aggregation for streaming data.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `operation` | String | ✅ Yes | - | Aggregation operation |
| `column` | String | No | - | Column to aggregate (some ops require) |
| `group_by` | Array | No | `[]` | Columns to group by |

**Operations:**
- `count`: Count records
- `sum`, `avg`, `min`, `max`: Numeric aggregations (require `column`)

**Example:**

```toml
[[stages]]
id = "aggregate"
function = "aggregate.stream"
inputs = ["windowed"]

[stages.config]
operation = "sum"
column = "value"
group_by = ["category"]
```

---

## Sinks

### csv.write

Write data to CSV files.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `path` | String | ✅ Yes | - | Output CSV file path |
| `has_headers` | Boolean | No | `true` | Write headers |
| `delimiter` | String | No | `,` | Column delimiter |

**Example:**

```toml
[[stages]]
id = "save_csv"
function = "csv.write"
inputs = ["processed"]

[stages.config]
path = "output/results.csv"
has_headers = true
delimiter = ","
```

---

### json.write

Write data to JSON files.

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `path` | String | ✅ Yes | - | Output JSON file path |
| `format` | String | No | `records` | Format: `records`, `jsonl` |
| `pretty` | Boolean | No | `false` | Pretty-print JSON |

**Example:**

```toml
[[stages]]
id = "save_json"
function = "json.write"
inputs = ["processed"]

[stages.config]
path = "output/results.json"
format = "records"
pretty = true
```

---

### stdout.write

Write data to standard output (batch mode).

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `format` | String | No | `table` | Format: `table`, `json`, `jsonl`, `csv` |
| `limit` | Integer | No | - | Maximum rows to display |

**Example:**

```toml
[[stages]]
id = "display"
function = "stdout.write"
inputs = ["results"]

[stages.config]
format = "table"
limit = 10
```

---

### stdout.stream

Write data to standard output (streaming mode).

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `format` | String | No | `jsonl` | Format: `jsonl`, `json`, `table` |

**Example:**

```toml
[[stages]]
id = "stream_output"
function = "stdout.stream"
inputs = ["processed"]

[stages.config]
format = "jsonl"
```

---

## Plugin Modules

### HTTP Plugin

**Plugin:** `http`

**Function:** `http` (source or sink, determined by stage inputs)

**Enable:**

```toml
[global]
plugins = ["http"]
```

**Source Example (fetch data):**

```toml
[[stages]]
id = "fetch_api"
function = "http"
inputs = []

[stages.config]
url = "https://api.example.com/data"
method = "GET"
format = "json"
timeout_seconds = 30

[stages.config.headers]
Authorization = "Bearer ${API_TOKEN}"
Accept = "application/json"
```

**Sink Example (send data):**

```toml
[[stages]]
id = "send_webhook"
function = "http"
inputs = ["data"]

[stages.config]
url = "https://webhook.example.com/events"
method = "POST"
format = "json"

[stages.config.headers]
Content-Type = "application/json"
```

**Configuration:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `url` | String | ✅ Yes | - | API endpoint URL |
| `method` | String | No | `GET` for source, `POST` for sink | HTTP method (GET, POST, PUT, PATCH, DELETE) |
| `format` | String | No | `json` | Data format: `json`, `jsonl`, `raw` |
| `headers` | Object | No | `{}` | Custom HTTP headers |
| `timeout_seconds` | Integer | No | `30` | Request timeout |

See [Plugin System](plugin-system.md#http-plugin) for detailed documentation.

---

### MongoDB Plugin

**Plugin:** `mongodb`

**Functions:**
- **Sources:**
  - `mongodb.find` - Find multiple documents
  - `mongodb.findOne` - Find single document
- **Sinks:**
  - `mongodb.insertOne` - Insert single document
  - `mongodb.insertMany` - Insert multiple documents
  - `mongodb.updateOne` - Update single document
  - `mongodb.updateMany` - Update multiple documents
  - `mongodb.deleteOne` - Delete single document
  - `mongodb.deleteMany` - Delete multiple documents
  - `mongodb.replaceOne` - Replace single document
  - `mongodb.replaceMany` - Replace multiple documents

**Enable:**

```toml
[global]
plugins = ["mongodb"]
```

**Source Example (find):**

```toml
[[stages]]
id = "load_users"
function = "mongodb.find"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "status": "active" }'
limit = 1000
```

**Sink Example (insertMany):**

```toml
[[stages]]
id = "save_to_mongo"
function = "mongodb.insertMany"
inputs = ["processed"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "results"
collection = "analytics"
```

**Update Example:**

```toml
[[stages]]
id = "update_status"
function = "mongodb.updateMany"
inputs = ["updates"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "status": "pending" }'
```

**Common Configuration (all operations):**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `uri` | String | ✅ Yes | - | MongoDB connection URI |
| `database` | String | ✅ Yes | - | Database name |
| `collection` | String | ✅ Yes | - | Collection name |

**Additional Options (find operations):**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `query` | String | No | `{}` | MongoDB query (JSON string) |
| `limit` | Integer | No | - | Maximum documents to fetch |

**Additional Options (update/delete/replace operations):**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `query` | String | ✅ Yes | - | MongoDB query filter (JSON string) |

See [Plugin System](plugin-system.md#mongodb-plugin) for detailed documentation.

---

## Data Format Conversion

Conveyor automatically converts between data formats:

```rust
pub enum DataFormat {
    DataFrame(DataFrame),      // Polars DataFrame (columnar)
    RecordBatch(RecordBatch),  // Vec<HashMap> (row-based)
    Raw(Vec<u8>),              // Raw bytes
    Stream(Stream),            // Async stream
}
```

**Conversion rules:**
- CSV/JSON sources → DataFrame
- HTTP/MongoDB → RecordBatch or DataFrame
- Transforms work with any format (auto-convert)
- Sinks accept any format (auto-convert)
- Streaming sources → Stream

---

## Module Discovery

List available modules:

```bash
# List all functions
conveyor list

# List by type
conveyor list --module-type sources
conveyor list --module-type transforms
conveyor list --module-type sinks
```

---

## See Also

- [DAG Pipelines](dag-pipelines.md) - Pipeline composition
- [HTTP Fetch Transform](http-fetch-transform.md) - Dynamic API calls
- [Plugin System](plugin-system.md) - Plugin development
- [Configuration](configuration.md) - Pipeline configuration
