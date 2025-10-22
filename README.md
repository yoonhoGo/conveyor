# Conveyor

[![CI](https://github.com/yoonhoGo/conveyor/actions/workflows/ci.yml/badge.svg)](https://github.com/yoonhoGo/conveyor/actions/workflows/ci.yml)
[![Release](https://github.com/yoonhoGo/conveyor/actions/workflows/release.yml/badge.svg)](https://github.com/yoonhoGo/conveyor/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)

A high-performance, TOML-based ETL (Extract, Transform, Load) CLI tool built in Rust for data pipeline processing.

## Why Conveyor?

- **⚡ 10-100x Faster**: Built with Rust and Polars for exceptional performance
- **📋 Simple Configuration**: Declarative TOML pipelines, no code required
- **🔀 Flexible DAG Pipelines**: Compose stages with automatic dependency resolution and parallel execution
- **🔌 Extensible**: Dynamic plugin system (FFI + WASM) for custom sources and sinks
- **🌊 Stream Processing**: Real-time data processing with windowing and aggregation
- **🤖 AI-Powered**: Built-in LLM transforms (OpenAI, Anthropic, OpenRouter, Ollama)
- **📊 Self-Documenting**: Interactive CLI guides you through pipeline creation

## Quick Start

### Installation

```bash
curl -fsSL https://raw.githubusercontent.com/yoonhoGo/conveyor/main/install.sh | bash
```

Or download from [releases](https://github.com/yoonhoGo/conveyor/releases).

### Your First Pipeline

Create `pipeline.toml`:

```toml
[pipeline]
name = "sales_analysis"
version = "1.0.0"

# Load CSV data
[[stages]]
id = "load"
function = "csv.read"
inputs = []
[stages.config]
path = "sales.csv"

# Filter high-value transactions
[[stages]]
id = "filter"
function = "filter.apply"
inputs = ["load"]
[stages.config]
column = "amount"
operator = ">="
value = 1000.0

# Save to JSON
[[stages]]
id = "save"
function = "json.write"
inputs = ["filter"]
[stages.config]
path = "high_value_sales.json"
pretty = true
```

Run it:

```bash
conveyor run pipeline.toml
```

## Core Features

### 1. Self-Documenting CLI

Discover functions without reading docs:

```bash
# List all available functions
conveyor list

# Get detailed info for any function
conveyor info csv.read

# Build a stage interactively
conveyor build
```

### 2. DAG-Based Pipelines

Compose flexible pipelines with branching and parallelism:

```toml
# Fan-out: Save data to multiple destinations
[[stages]]
id = "data"
function = "csv.read"
inputs = []

[[stages]]
id = "save_json"
function = "json.write"
inputs = ["data"]  # Parallel

[[stages]]
id = "save_csv"
function = "csv.write"
inputs = ["data"]  # Parallel
```

See [DAG Pipelines Guide](docs/dag-pipelines.md).

### 3. Plugin System

Extend Conveyor with plugins loaded on-demand:

```toml
[global]
plugins = ["http", "mongodb"]

[[stages]]
id = "api_data"
function = "http.get"
inputs = []
[stages.config]
url = "https://api.example.com/data"
```

Available plugins:
- **HTTP**: REST API integration ([docs](docs/plugins/http.md))
- **MongoDB**: Database operations ([docs](docs/plugins/mongodb.md))

Create your own: [Plugin Development Guide](docs/plugin-system.md)

### 4. Stream Processing

Process real-time data streams:

```bash
# Stream processing with windowing
tail -f app.log | conveyor run stream_pipeline.toml
```

Features:
- Tumbling, sliding, and session windows
- Real-time aggregations (count, sum, avg, min, max)
- Micro-batching for efficiency

### 5. AI-Powered Transforms

Add LLM intelligence to your pipelines:

```toml
[[stages]]
id = "classify"
function = "ai.generate"
inputs = ["reviews"]
[stages.config]
provider = "openai"
model = "gpt-4"
prompt = "Classify sentiment (positive/negative/neutral): {{text}}"
output_column = "sentiment"
```

Supports: OpenAI, Anthropic, OpenRouter, Ollama

## Available Functions

### Built-in Functions

**Sources**: `csv.read`, `json.read`, `stdin.read`, `stdin.stream`, `file.watch`

**Transforms**: `filter.apply`, `map.apply`, `select.apply`, `groupby.apply`, `sort.apply`, `distinct.apply`, `json.extract`, `ai.generate`, `validate.schema`, `http.fetch`, `reduce.apply`, `window.apply`, `aggregate.stream`

**Sinks**: `csv.write`, `json.write`, `stdout.write`, `stdout.stream`

📖 [Complete Function Reference](docs/builtin-functions.md)

### Plugin Functions

**HTTP Plugin**: `http.get`, `http.post`, `http.put`, `http.patch`, `http.delete` ([docs](docs/plugins/http.md))

**MongoDB Plugin**: `mongodb.find`, `mongodb.findOne`, `mongodb.insertOne`, `mongodb.insertMany`, `mongodb.updateOne`, `mongodb.updateMany`, `mongodb.deleteOne`, `mongodb.deleteMany`, `mongodb.replaceOne`, `mongodb.replaceMany` ([docs](docs/plugins/mongodb.md))

## Documentation

### For Users
- 📖 [CLI Reference](docs/cli-reference.md) - All commands and options
- ⚙️ [Configuration Guide](docs/configuration.md) - TOML configuration reference
- 🔀 [DAG Pipelines](docs/dag-pipelines.md) - Pipeline composition and execution
- 📦 [Built-in Functions](docs/builtin-functions.md) - Sources, transforms, sinks
- 🔌 [HTTP Plugin](docs/plugins/http.md) - REST API integration
- 🍃 [MongoDB Plugin](docs/plugins/mongodb.md) - Database operations
- 🌐 [HTTP Fetch Transform](docs/http-fetch-transform.md) - Dynamic API calls
- 📊 [Metadata System](docs/metadata-system.md) - Self-documenting features

### For Developers
- 🛠️ [Development Guide](docs/development.md) - Build, test, contribute
- 🔧 [Plugin Development](docs/plugin-system.md) - Create custom plugins
- 🏗️ [Architecture (CLAUDE.md)](CLAUDE.md) - Technical implementation details

## Examples

See [examples/](examples/) directory for complete pipeline examples:

1. **CSV to JSON**: Simple data transformation
2. **API Integration**: Fetch and process external data
3. **Data Enrichment**: Combine data from multiple sources
4. **Stream Processing**: Real-time data analysis
5. **AI Classification**: Sentiment analysis with LLMs

## Performance

Built for speed:

- **Rust**: Zero-cost abstractions, memory safety
- **Polars**: 10-100x faster than Pandas
- **Tokio**: Efficient async I/O
- **Arrow**: Columnar memory format

## Development

```bash
git clone https://github.com/yoonhoGo/conveyor.git
cd conveyor
cargo build --all
cargo test --all
```

See [Development Guide](docs/development.md) for details.

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Areas for contribution:
- New plugins (databases, cloud services, message queues)
- Additional transforms
- Performance optimizations
- Documentation and examples

## Roadmap

### Completed ✅
- Core DAG pipeline engine
- CSV, JSON, HTTP data sources
- Dynamic plugin system (FFI & WASM)
- Stream processing with windowing
- AI-powered transforms
- Self-documenting CLI

### In Progress 🚧
- MongoDB plugin enhancements
- WASM plugin ecosystem
- Advanced streaming joins

### Planned 📋
- PostgreSQL, MySQL plugins
- Advanced transforms (join, pivot)
- Exactly-once processing guarantees
- Kafka, Redis Streams integration
- Web UI for monitoring
- Distributed execution

## License

MIT License - see [LICENSE](LICENSE) for details.

## Support

- 📖 [Documentation](docs/)
- 🐛 [Issue Tracker](https://github.com/yoonhoGo/conveyor/issues)
- 💬 [Discussions](https://github.com/yoonhoGo/conveyor/discussions)

---

**Made with ❤️ and Rust** | **Built with Claude Code**
