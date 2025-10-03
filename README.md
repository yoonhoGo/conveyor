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
type = "source.csv"
inputs = []

[stages.config]
path = "data/input.csv"

# Stage 2: Filter data
[[stages]]
id = "filter"
type = "transform.filter"
inputs = ["load_data"]

[stages.config]
column = "amount"
operator = ">="
value = 100.0

# Stage 3: Save to JSON
[[stages]]
id = "save"
type = "sink.json"
inputs = ["filter"]

[stages.config]
path = "output/result.json"
pretty = true
```

Run it:

```bash
conveyor run -c pipeline.toml
```

## Documentation

- **[DAG Pipelines](docs/dag-pipelines.md)** - Flexible pipeline composition with branching and parallelism
- **[HTTP Fetch Transform](docs/http-fetch-transform.md)** - Dynamic API calls within pipelines
- **[Plugin System](docs/plugin-system.md)** - Create and use plugins (FFI & WASM)
- **[Modules Reference](docs/modules-reference.md)** - All built-in sources, transforms, and sinks
- **[Configuration](docs/configuration.md)** - Complete configuration reference
- **[Architecture](docs/architecture.md)** - System design and internals
- **[Development](docs/development.md)** - Building and contributing

## Examples

### 1. CSV to JSON Transformation

```toml
[[stages]]
id = "load"
type = "source.csv"
inputs = []
[stages.config]
path = "sales.csv"

[[stages]]
id = "save"
type = "sink.json"
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
type = "plugin.http"
inputs = []
[stages.config]
url = "https://api.example.com/data"
method = "GET"

[[stages]]
id = "save"
type = "sink.json"
inputs = ["fetch"]
[stages.config]
path = "api_data.json"
```

### 3. Data Enrichment with API

```toml
[[stages]]
id = "users"
type = "source.csv"
inputs = []

[[stages]]
id = "fetch_profiles"
type = "transform.http_fetch"
inputs = ["users"]
[stages.config]
url = "https://api.example.com/users/{{ id }}/profile"
result_field = "profile"

[[stages]]
id = "save"
type = "sink.json"
inputs = ["fetch_profiles"]
[stages.config]
path = "enriched_users.json"
```

### 4. Real-time Stream Processing

```toml
[[stages]]
id = "stream_source"
type = "source.stdin_stream"
inputs = []
[stages.config]
format = "jsonl"

[[stages]]
id = "filter"
type = "transform.filter"
inputs = ["stream_source"]
[stages.config]
column = "value"
operator = ">="
value = 100.0

[[stages]]
id = "output"
type = "sink.stdout_stream"
inputs = ["filter"]
[stages.config]
format = "jsonl"
```

### 5. Windowing and Aggregation

```toml
[[stages]]
id = "stream_source"
type = "source.stdin_stream"
inputs = []
[stages.config]
format = "jsonl"

[[stages]]
id = "window"
type = "transform.window"
inputs = ["stream_source"]
[stages.config]
type = "tumbling"
size = 10  # 10 records per window

[[stages]]
id = "aggregate"
type = "transform.aggregate_stream"
inputs = ["window"]
[stages.config]
operation = "count"
group_by = ["level"]

[[stages]]
id = "output"
type = "sink.stdout_stream"
inputs = ["aggregate"]
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
- `source.csv` - Read CSV files
- `source.json` - Read JSON files
- `source.stdin` - Read from standard input
- `source.stdin_stream` - Streaming stdin (line-by-line)
- `source.file_watch` - Monitor files for changes (polling-based)

### Built-in Transforms
- `transform.filter` - Filter rows by conditions
- `transform.map` - Transform columns with expressions
- `transform.validate_schema` - Validate data schema
- `transform.http_fetch` - Make HTTP requests per row
- `transform.window` - Apply windowing (tumbling/sliding/session)
- `transform.aggregate_stream` - Real-time aggregation (count/sum/avg/min/max)

### Built-in Sinks
- `sink.csv` - Write to CSV
- `sink.json` - Write to JSON
- `sink.stdout` - Display in terminal
- `sink.stdout_stream` - Real-time streaming output

### Plugin Modules
- `plugin.http` - HTTP source & sink (REST APIs)
- `plugin.mongodb` - MongoDB source & sink

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
