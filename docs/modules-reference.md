# Modules Reference

Complete index of all available modules (sources, transforms, sinks) in Conveyor.

## Quick Navigation

- **[Built-in Functions](builtin-functions.md)** - Detailed documentation for all built-in sources, transforms, and sinks
- **[HTTP Plugin](plugins/http.md)** - REST API integration
- **[MongoDB Plugin](plugins/mongodb.md)** - MongoDB database operations

## Built-in Sources

Read data from various sources.

| Function | Description | Documentation |
|----------|-------------|---------------|
| `csv.read` | Read data from CSV files | [Details](builtin-functions.md#csvread) |
| `json.read` | Read data from JSON files | [Details](builtin-functions.md#jsonread) |
| `stdin.read` | Read from standard input (batch) | [Details](builtin-functions.md#stdinread) |
| `stdin.stream` | Read from standard input (streaming) | [Details](builtin-functions.md#stdinstream) |
| `file.watch` | Monitor file for changes (polling) | [Details](builtin-functions.md#filewatch) |

## Built-in Transforms

Transform and process data.

| Function | Description | Documentation |
|----------|-------------|---------------|
| `filter.apply` | Filter rows based on conditions | [Details](builtin-functions.md#filterapply) |
| `map.apply` | Transform columns with expressions | [Details](builtin-functions.md#mapapply) |
| `select.apply` | Select specific columns | [Details](builtin-functions.md#selectapply) |
| `groupby.apply` | Group and aggregate data | [Details](builtin-functions.md#groupbyapply) |
| `sort.apply` | Sort by columns | [Details](builtin-functions.md#sortapply) |
| `distinct.apply` | Remove duplicates | [Details](builtin-functions.md#distinctapply) |
| `json.extract` | Extract nested JSON fields | [Details](builtin-functions.md#jsonextract) |
| `ai.generate` | LLM-powered transformations | [Details](builtin-functions.md#aigenerate) |
| `validate.schema` | Validate data schema and types | [Details](builtin-functions.md#validateschema) |
| `http.fetch` | Make HTTP requests per row | [Details](builtin-functions.md#httpfetch), [Advanced](http-fetch-transform.md) |
| `reduce.apply` | Reduce to single aggregated value | [Details](builtin-functions.md#reduceapply) |
| `window.apply` | Apply windowing (streaming) | [Details](builtin-functions.md#windowapply) |
| `aggregate.stream` | Real-time aggregation | [Details](builtin-functions.md#aggregatestream) |

## Built-in Sinks

Write data to various destinations.

| Function | Description | Documentation |
|----------|-------------|---------------|
| `csv.write` | Write to CSV files | [Details](builtin-functions.md#csvwrite) |
| `json.write` | Write to JSON files | [Details](builtin-functions.md#jsonwrite) |
| `stdout.write` | Display in terminal (batch) | [Details](builtin-functions.md#stdoutwrite) |
| `stdout.stream` | Real-time streaming output | [Details](builtin-functions.md#stdoutstream) |

## Plugin Modules

### HTTP Plugin

REST API integration. See [HTTP Plugin Documentation](plugins/http.md).

**Sources:**
- `http.get` - Fetch data (GET request)

**Sinks:**
- `http.post` - Send data (POST request)
- `http.put` - Update data (PUT request)
- `http.patch` - Partial update (PATCH request)
- `http.delete` - Delete data (DELETE request)

**Enable:**
```toml
[global]
plugins = ["http"]
```

### MongoDB Plugin

MongoDB database operations. See [MongoDB Plugin Documentation](plugins/mongodb.md).

**Sources (Read Operations):**
- `mongodb.find` - Find multiple documents
- `mongodb.findOne` - Find single document

**Sinks (Write Operations):**
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

## Discovering Functions

Use the CLI to explore available functions:

```bash
# List all functions with descriptions
conveyor list

# Filter by type
conveyor list -t sources
conveyor list -t transforms
conveyor list -t sinks

# Get detailed information about a specific function
conveyor info csv.read
conveyor info filter.apply
conveyor info mongodb.find

# Build a stage interactively with guided prompts
conveyor build
```

## Global Configuration

### Variables System

Define reusable variables with environment substitution:

```toml
[global.variables]
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.example.com"
```

Use in stages:

```toml
[stages.config]
url = "{{base_url}}/endpoint"
headers = { "Authorization" = "Bearer {{api_key}}" }
```

See [Built-in Functions - Global Configuration](builtin-functions.md#global-configuration) for details.

## Data Formats

Conveyor automatically converts between data formats:

- **DataFrame**: Polars DataFrame (columnar)
- **RecordBatch**: Vec<HashMap> (row-based)
- **Raw**: Raw bytes
- **Stream**: Async stream

See [Built-in Functions - Data Format Conversion](builtin-functions.md#data-format-conversion) for details.

## See Also

- **[CLI Reference](cli-reference.md)** - All CLI commands and options
- **[Configuration Guide](configuration.md)** - TOML configuration reference
- **[DAG Pipelines](dag-pipelines.md)** - Pipeline composition and execution
- **[Built-in Functions](builtin-functions.md)** - Detailed function documentation
- **[HTTP Plugin](plugins/http.md)** - REST API integration guide
- **[MongoDB Plugin](plugins/mongodb.md)** - MongoDB operations guide
- **[HTTP Fetch Transform](http-fetch-transform.md)** - Dynamic API calls
- **[Metadata System](metadata-system.md)** - Self-documenting features
- **[Plugin Development](plugin-system.md)** - Creating custom plugins
