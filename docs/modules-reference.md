# Modules Reference

Complete reference for all built-in and plugin modules in Conveyor.

## Built-in Data Sources

### CSV Source

Read data from CSV files.

**Type:** `source.csv`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `path` | ✅ Yes | - | Path to CSV file |
| `headers` | No | `true` | First row contains headers |
| `delimiter` | No | `,` | Column delimiter |

**Example:**

```toml
[[stages]]
id = "load_csv"
type = "source.csv"
inputs = []

[stages.config]
path = "data/sales.csv"
headers = true
delimiter = ","
```

### JSON Source

Read data from JSON files.

**Type:** `source.json`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `path` | ✅ Yes | - | Path to JSON file |
| `format` | No | `records` | Format: `records`, `jsonl`, `dataframe` |

**Example:**

```toml
[[stages]]
id = "load_json"
type = "source.json"
inputs = []

[stages.config]
path = "data/users.json"
format = "records"  # Array of objects
```

**Formats:**
- `records`: JSON array of objects `[{...}, {...}]`
- `jsonl`: Newline-delimited JSON (one object per line)
- `dataframe`: Polars DataFrame JSON format

### Stdin Source

Read data from standard input.

**Type:** `source.stdin`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `format` | No | `json` | Format: `json`, `jsonl`, `csv`, `raw` |

**Example:**

```toml
[[stages]]
id = "read_stdin"
type = "source.stdin"
inputs = []

[stages.config]
format = "jsonl"
```

**Usage:**
```bash
cat data.json | conveyor run -c pipeline.toml
```

## Built-in Transforms

### Filter Transform

Filter rows based on conditions.

**Type:** `transform.filter`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `column` | ✅ Yes | - | Column name to filter on |
| `operator` | ✅ Yes | - | Comparison operator |
| `value` | ✅ Yes | - | Value to compare against |

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
type = "transform.filter"
inputs = ["data"]

[stages.config]
column = "amount"
operator = ">="
value = 1000.0
```

### Map Transform

Create or transform columns using expressions.

**Type:** `transform.map`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `expression` | ✅ Yes | - | Mathematical expression |
| `output_column` | ✅ Yes | - | Name of the output column |

**Supported Operators:**
- `+`, `-`, `*`, `/`: Arithmetic
- Column references by name

**Example:**

```toml
[[stages]]
id = "add_tax"
type = "transform.map"
inputs = ["sales"]

[stages.config]
expression = "amount * 1.1"
output_column = "amount_with_tax"
```

### Validate Schema Transform

Validate data schema and types.

**Type:** `transform.validate_schema`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `required_fields` | No | `[]` | List of required field names |
| `field_types` | No | `{}` | Field name → type mapping |
| `non_nullable` | No | `[]` | Fields that cannot be null |
| `unique` | No | `[]` | Fields that must be unique |

**Field Types:**
- `string`, `int`, `float`, `bool`, `date`

**Example:**

```toml
[[stages]]
id = "validate"
type = "transform.validate_schema"
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

### HTTP Fetch Transform

Make HTTP requests using row data.

**Type:** `transform.http_fetch`

See [HTTP Fetch Transform](http-fetch-transform.md) for detailed documentation.

**Example:**

```toml
[[stages]]
id = "fetch_api"
type = "transform.http_fetch"
inputs = ["users"]

[stages.config]
url = "https://api.example.com/users/{{ id }}/posts"
method = "GET"
result_field = "posts"
```

## Built-in Sinks

### CSV Sink

Write data to CSV files.

**Type:** `sink.csv`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `path` | ✅ Yes | - | Output CSV file path |
| `headers` | No | `true` | Write headers |
| `delimiter` | No | `,` | Column delimiter |

**Example:**

```toml
[[stages]]
id = "save_csv"
type = "sink.csv"
inputs = ["processed"]

[stages.config]
path = "output/results.csv"
headers = true
delimiter = ","
```

### JSON Sink

Write data to JSON files.

**Type:** `sink.json`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `path` | ✅ Yes | - | Output JSON file path |
| `format` | No | `records` | Format: `records`, `jsonl` |
| `pretty` | No | `false` | Pretty-print JSON |

**Example:**

```toml
[[stages]]
id = "save_json"
type = "sink.json"
inputs = ["processed"]

[stages.config]
path = "output/results.json"
format = "records"
pretty = true
```

### Stdout Sink

Write data to standard output.

**Type:** `sink.stdout`

**Configuration:**

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `format` | No | `table` | Format: `table`, `json`, `jsonl`, `csv` |
| `limit` | No | - | Maximum rows to display |

**Example:**

```toml
[[stages]]
id = "display"
type = "sink.stdout"
inputs = ["results"]

[stages.config]
format = "table"
limit = 10  # Show first 10 rows
```

## Plugin Modules

### HTTP Plugin (Source & Sink)

**Plugin:** `http`

See [Plugin System](plugin-system.md#http-plugin) for detailed documentation.

**Source Example:**

```toml
[global]
plugins = ["http"]

[[stages]]
id = "fetch_api"
type = "plugin.http"
inputs = []

[stages.config]
url = "https://api.example.com/data"
method = "GET"
format = "json"
```

**Sink Example:**

```toml
[[stages]]
id = "send_results"
type = "plugin.http"
inputs = ["data"]

[stages.config]
url = "https://api.example.com/webhook"
method = "POST"
format = "json"
```

### MongoDB Plugin (Source & Sink)

**Plugin:** `mongodb`

See [Plugin System](plugin-system.md#mongodb-plugin) for detailed documentation.

**Source Example:**

```toml
[global]
plugins = ["mongodb"]

[[stages]]
id = "load_events"
type = "plugin.mongodb"
inputs = []

[stages.config]
connection_string = "mongodb://localhost:27017"
database = "analytics"
collection = "events"
query = '{ "status": "active" }'
```

**Sink Example:**

```toml
[[stages]]
id = "save_to_mongo"
type = "plugin.mongodb"
inputs = ["processed"]

[stages.config]
connection_string = "mongodb://localhost:27017"
database = "results"
collection = "processed_data"
```

## Special Stages

### Pipeline Stage

Nest a complete pipeline within a stage.

**Type:** `stage.pipeline`

**Configuration:**

A pipeline stage has its own complete pipeline configuration nested within it.

**Example:**

```toml
[[stages]]
id = "nested_pipeline"
type = "stage.pipeline"
inputs = ["data"]

[stages.config]
# Nested pipeline configuration
[[stages.config.stages]]
id = "sub_filter"
type = "transform.filter"
inputs = []  # Uses parent input

[stages.config.stages.config]
column = "value"
operator = ">"
value = 100
```

## Data Format Conversion

Conveyor automatically converts between data formats:

```rust
pub enum DataFormat {
    DataFrame(DataFrame),      // Polars DataFrame (columnar)
    RecordBatch(RecordBatch),  // Vec<HashMap> (row-based)
    Raw(Vec<u8>),              // Raw bytes
}
```

**Conversion rules:**
- CSV/JSON sources → DataFrame
- HTTP/MongoDB → RecordBatch or DataFrame
- Transforms work with any format (auto-convert)
- Sinks accept any format (auto-convert)

## Module Discovery

List available modules:

```bash
# List all modules
conveyor list

# List sources only
conveyor list --module-type sources

# List transforms only
conveyor list --module-type transforms

# List sinks only
conveyor list --module-type sinks
```

## See Also

- [DAG Pipelines](dag-pipelines.md) - Pipeline composition
- [HTTP Fetch Transform](http-fetch-transform.md) - Dynamic API calls
- [Plugin System](plugin-system.md) - Plugin modules
- [Configuration](configuration.md) - Pipeline configuration
