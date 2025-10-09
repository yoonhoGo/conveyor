# Conveyor

[![CI](https://github.com/yoonhoGo/conveyor/actions/workflows/ci.yml/badge.svg)](https://github.com/yoonhoGo/conveyor/actions/workflows/ci.yml)
[![Release](https://github.com/yoonhoGo/conveyor/actions/workflows/release.yml/badge.svg)](https://github.com/yoonhoGo/conveyor/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)

A high-performance, TOML-based ETL (Extract, Transform, Load) CLI tool built in Rust for data pipeline processing with a **dual plugin system** (FFI + WASM).

## Features

- **📋 TOML Configuration**: Simple, declarative pipeline definitions
- **🔀 DAG-Based Pipelines**: Flexible stage composition with automatic dependency resolution
- **⚡ High Performance**: Built with Rust and Polars for fast data processing (10-100x faster than Python)
- **🔌 Dynamic Plugin System**: Load plugins on-demand with version checking and panic isolation
- **🔄 Async Processing**: Built on Tokio for efficient concurrent operations
- **🌊 Stream Processing**: Real-time data processing with micro-batching and windowing support
- **🤖 AI Integration**: LLM-powered transforms with OpenAI, Anthropic, OpenRouter, and Ollama support
- **🔧 Global Variables**: Reusable variables with environment variable substitution
- **🛡️ Type-Safe**: Rust's type system ensures reliability and safety
- **📊 Multiple Data Formats**: Support for CSV, JSON, HTTP APIs, and more

## Quick Start

### Installation

**One-line installation:**

```bash
curl -fsSL https://raw.githubusercontent.com/yoonhoGo/conveyor/main/install.sh | bash
```

Or [download the latest release](https://github.com/yoonhoGo/conveyor/releases) for your platform.

**Build from source:**

```bash
git clone https://github.com/yoonhoGo/conveyor.git
cd conveyor
cargo build --release
```

### Basic Pipeline

Create `pipeline.toml`:

```toml
[pipeline]
name = "my_first_pipeline"
version = "1.0.0"

[global]
log_level = "info"

# Stage 1: Load CSV data
[[stages]]
id = "load_data"
function = "csv.read"
inputs = []

[stages.config]
path = "data/input.csv"

# Stage 2: Filter data
[[stages]]
id = "filter"
function = "filter.apply"
inputs = ["load_data"]

[stages.config]
column = "amount"
operator = ">="
value = 100.0

# Stage 3: Save to JSON
[[stages]]
id = "save"
function = "json.write"
inputs = ["filter"]

[stages.config]
path = "output/result.json"
pretty = true
```

Run it:

```bash
conveyor run pipeline.toml
```

## CLI Commands

Conveyor provides an intuitive CLI with self-documenting features:

### Core Commands

```bash
# Run a pipeline
conveyor run pipeline.toml

# Validate configuration without running
conveyor validate pipeline.toml
conveyor run pipeline.toml --dry-run

# List all available functions
conveyor list
conveyor list -t sources     # Filter by type

# Get detailed help for any function
conveyor info csv.read
conveyor info filter.apply

# Build a stage interactively (guided prompts)
conveyor build
```

### Discovery Workflow

```bash
# 1. Explore available functions with descriptions
conveyor list

# Output:
# SOURCES:
# ----------------------------------------------------------------------
#   • csv.read                  - Read data from CSV files
#   • json.read                 - Read data from JSON files
#   ...

# 2. Get detailed information about a function
conveyor info filter.apply

# Shows:
# - Full description
# - All parameters (required/optional)
# - Parameter types and validation rules
# - Usage examples
# - Tags for search

# 3. Build a stage interactively
conveyor build

# Interactive prompts guide you through:
# - Function selection
# - Stage configuration
# - Parameter input with validation
# - Generates ready-to-use TOML
```

### Stage Management

```bash
# Create a new pipeline template
conveyor stage new -o pipeline.toml

# Add a stage to existing pipeline
conveyor stage add pipeline.toml

# Edit pipeline interactively
conveyor stage edit pipeline.toml

# Export function metadata as JSON
conveyor stage describe csv.read
```

### Self-Documenting Features

Every function has rich metadata including:
- **Description**: Clear explanation of what it does
- **Parameters**: Type, default value, validation rules
- **Examples**: Real-world usage scenarios
- **Tags**: Searchable categorization

No need to search through documentation - all information is built into the CLI!

## Documentation

### User Guides
- **[CLI Reference](docs/cli-reference.md)** - Complete CLI command documentation
- **[Metadata System](docs/metadata-system.md)** - Self-documenting features and how to use them
- **[DAG Pipelines](docs/dag-pipelines.md)** - Flexible pipeline composition with branching and parallelism
- **[HTTP Fetch Transform](docs/http-fetch-transform.md)** - Dynamic API calls within pipelines
- **[Modules Reference](docs/modules-reference.md)** - All built-in sources, transforms, and sinks
- **[Configuration](docs/configuration.md)** - Complete configuration reference

### Developer Guides
- **[Plugin System](docs/plugin-system.md)** - Create and use plugins (FFI & WASM)
- **[Architecture](docs/architecture.md)** - System design and internals
- **[Development](docs/development.md)** - Building and contributing
- **[CLAUDE.md](CLAUDE.md)** - Technical implementation notes for AI agents

## Examples

### 1. CSV to JSON Transformation

```toml
[[stages]]
id = "load"
function = "csv.read"
inputs = []
[stages.config]
path = "sales.csv"

[[stages]]
id = "save"
function = "json.write"
inputs = ["load"]
[stages.config]
path = "output.json"
```

### 2. HTTP API Pipeline

```toml
[global]
plugins = ["http"]

[[stages]]
id = "fetch"
function = "http.get"
inputs = []
[stages.config]
url = "https://api.example.com/data"

[[stages]]
id = "save"
function = "json.write"
inputs = ["fetch"]
[stages.config]
path = "api_data.json"
```

### 3. Data Enrichment with API

```toml
[[stages]]
id = "users"
function = "csv.read"
inputs = []

[[stages]]
id = "fetch_profiles"
function = "http.fetch"
inputs = ["users"]
[stages.config]
url = "https://api.example.com/users/{{ id }}/profile"
result_field = "profile"

[[stages]]
id = "save"
function = "json.write"
inputs = ["fetch_profiles"]
[stages.config]
path = "enriched_users.json"
```

### 4. Real-time Stream Processing

```toml
[[stages]]
id = "stream_source"
function = "stdin.stream"
inputs = []
[stages.config]
format = "jsonl"

[[stages]]
id = "filter"
function = "filter.apply"
inputs = ["stream_source"]
[stages.config]
column = "value"
operator = ">="
value = 100.0

[[stages]]
id = "output"
function = "stdout.stream"
inputs = ["filter"]
[stages.config]
format = "jsonl"
```

### 5. Windowing and Aggregation

```toml
[[stages]]
id = "stream_source"
function = "stdin.stream"
inputs = []
[stages.config]
format = "jsonl"

[[stages]]
id = "window"
function = "window.apply"
inputs = ["stream_source"]
[stages.config]
type = "tumbling"
size = 10  # 10 records per window

[[stages]]
id = "aggregate"
function = "aggregate.stream"
inputs = ["window"]
[stages.config]
operation = "count"
group_by = ["level"]

[[stages]]
id = "output"
function = "stdout.stream"
inputs = ["aggregate"]
```

### 6. AI-Powered Data Processing

```toml
[global.variables]
api_key = "${OPENAI_API_KEY}"

[[stages]]
id = "load"
function = "csv.read"
inputs = []
[stages.config]
path = "reviews.csv"

[[stages]]
id = "classify"
function = "ai.generate"
inputs = ["load"]
[stages.config]
provider = "openai"
model = "gpt-4"
prompt = "Classify sentiment (positive/negative/neutral): {{text}}"
output_column = "sentiment"
max_tokens = 10

[[stages]]
id = "save"
function = "json.write"
inputs = ["classify"]
[stages.config]
path = "classified_reviews.json"
```

### 7. JSON Field Extraction

```toml
[[stages]]
id = "load"
function = "csv.read"
inputs = []
[stages.config]
path = "logs.csv"

[[stages]]
id = "extract"
function = "json.extract"
inputs = ["load"]
[stages.config]
column = "payload"
path = "meta.req.headers.user-agent"
output_column = "user_agent"

[[stages]]
id = "save"
function = "stdout.write"
inputs = ["extract"]
```

See [examples/](examples/) for more complete pipelines.

## CLI Commands

```bash
# Run pipeline
conveyor run -c pipeline.toml

# Validate configuration
conveyor validate -c pipeline.toml

# List available modules
conveyor list

# Create new pipeline
conveyor stage new -o my-pipeline.toml

# Add stage to pipeline
conveyor stage add pipeline.toml

# Interactive pipeline editor
conveyor stage edit pipeline.toml
```

## Available Modules

### Built-in Sources
- `csv.read` - Read CSV files
- `json.read` - Read JSON files
- `stdin.read` - Read from standard input
- `stdin.stream` - Streaming stdin (line-by-line)
- `file.watch` - Monitor files for changes (polling-based)

### Built-in Transforms
- `filter.apply` - Filter rows by conditions
- `map.apply` - Transform columns with expressions
- `select.apply` - Select specific columns
- `groupby.apply` - Group and aggregate data
- `sort.apply` - Sort by columns
- `distinct.apply` - Remove duplicates
- `json.extract` - Extract nested JSON fields
- `ai.generate` - LLM-powered transformations (OpenAI, Anthropic, OpenRouter, Ollama)
- `validate.schema` - Validate data schema
- `http.fetch` - Make HTTP requests per row
- `reduce.apply` - Reduce to single value
- `window.apply` - Apply windowing (tumbling/sliding/session)
- `aggregate.stream` - Real-time aggregation (count/sum/avg/min/max)

### Built-in Sinks
- `csv.write` - Write to CSV
- `json.write` - Write to JSON
- `stdout.write` - Display in terminal
- `stdout.stream` - Real-time streaming output

### Plugin Modules
- `http.get`, `http.post`, `http.put`, `http.patch`, `http.delete` - HTTP operations
- `mongodb.find`, `mongodb.insert` - MongoDB operations

See [Modules Reference](docs/modules-reference.md) for complete documentation.

## Plugin System

Conveyor supports dynamic plugins loaded on-demand:

```toml
[global]
plugins = ["http", "mongodb"]  # Load only what you need
```

**Available Plugins:**
- **HTTP Plugin**: REST API integration
- **MongoDB Plugin**: MongoDB database operations

**Create Custom Plugins:**
- FFI Plugins (Rust cdylib)
- WASM Plugins (Component Model)

See [Plugin System](docs/plugin-system.md) for plugin development guide.

## Performance

Conveyor is built for performance:

- **Rust**: Zero-cost abstractions and memory safety
- **Polars**: 10-100x faster than Pandas for data processing
- **Tokio**: Efficient async I/O operations
- **Arrow**: Columnar memory format for optimal performance

## Development

```bash
# Clone repository
git clone https://github.com/yoonhoGo/conveyor.git
cd conveyor

# Build all crates
cargo build --all

# Run tests
cargo test --all

# Run clippy
cargo clippy --all-targets --all-features

# Install git hooks (recommended)
./hooks/install.sh
```

**Git Hooks**: The pre-push hook automatically runs lint checks before pushing. See [hooks/README.md](hooks/README.md) for details.

See [Development Guide](docs/development.md) for detailed instructions.

## Contributing

Contributions are welcome! Areas for contribution:

- **New Plugins**: Database connectors, cloud services, message queues
- **Transforms**: More data transformation functions
- **Performance**: Optimizations and benchmarks
- **Documentation**: Examples, guides, tutorials

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Roadmap

### Completed ✅
- Core DAG pipeline engine
- CSV, JSON, HTTP data sources
- Dynamic plugin system (FFI & WASM)
- HTTP fetch transform
- CLI with interactive stage management
- Stream processing infrastructure (DataFormat::Stream, micro-batching, windowing)
- Streaming sources (stdin_stream, file_watch)
- Streaming sinks (stdout_stream)
- Streaming transforms (window, aggregate_stream)
- Data manipulation transforms (groupby, sort, select, distinct)
- JSON extraction from nested fields
- AI-powered transforms (OpenAI, Anthropic, OpenRouter, Ollama)
- Global variables with environment substitution

### In Progress 🚧
- MongoDB plugin
- WASM plugin ecosystem
- Advanced streaming joins

### Planned 📋
- PostgreSQL, MySQL plugins
- Advanced transforms (join, pivot)
- Exactly-once processing guarantees
- Checkpointing and recovery for streams
- Event-driven file watching (inotify/FSEvents)
- Kafka, Redis Streams integration
- Web UI for monitoring
- Distributed execution

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Powered by [Polars](https://www.pola.rs/) for data processing
- Uses [Tokio](https://tokio.rs/) for async runtime
- CLI built with [Clap](https://docs.rs/clap/)

## Support

- 📖 [Documentation](docs/)
- 🐛 [Issue Tracker](https://github.com/yoonhoGo/conveyor/issues)
- 💬 [Discussions](https://github.com/yoonhoGo/conveyor/discussions)

---

**Made with ❤️ and Rust** | **Built with Claude Code**
