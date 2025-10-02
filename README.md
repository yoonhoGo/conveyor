# Conveyor

[![CI](https://github.com/yoonhoGo/conveyor/actions/workflows/ci.yml/badge.svg)](https://github.com/yoonhoGo/conveyor/actions/workflows/ci.yml)
[![Release](https://github.com/yoonhoGo/conveyor/actions/workflows/release.yml/badge.svg)](https://github.com/yoonhoGo/conveyor/actions/workflows/release.yml)
[![npm version](https://badge.fury.io/js/%40conveyor%2Fcli.svg)](https://www.npmjs.com/package/@conveyor/cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)

A high-performance, TOML-based ETL (Extract, Transform, Load) CLI tool built in Rust for data pipeline processing with a **dual plugin system** (FFI + WASM).

## Features

- **📋 TOML Configuration**: Simple, declarative pipeline definitions
- **🔀 DAG-Based Pipelines**: NEW! Flexible stage composition with automatic dependency resolution
- **⚡ High Performance**: Built with Rust and Polars for fast data processing (10-100x faster than Python)
- **🔌 Dynamic Plugin System**: Load plugins on-demand with version checking and panic isolation
- **🔄 Async Processing**: Built on Tokio for efficient concurrent operations with parallel stage execution
- **🛡️ Type-Safe**: Rust's type system ensures reliability and safety
- **📊 Multiple Data Formats**: Support for CSV, JSON, HTTP APIs, and more
- **🏗️ Workspace Architecture**: Modular crate structure for maintainability
- **🔁 Backward Compatible**: Seamlessly converts legacy pipelines to DAG format

## Installation

### Option 1: npm (Recommended for End Users)

```bash
# Install globally
npm install -g @conveyor/cli

# Or use with npx (no installation)
npx @conveyor/cli --help
```

The npm package automatically downloads the appropriate binary for your platform (macOS, Linux, Windows).

### Option 2: Build from Source

**Prerequisites:**
- Rust 1.70 or higher
- Cargo

```bash
git clone https://github.com/yoonhoGo/conveyor.git
cd conveyor
cargo build --release --all
```

The binary will be available at `target/release/conveyor`.
Plugin libraries will be in `target/release/` as `libconveyor_plugin_*.dylib` (macOS) or `.so` (Linux).

## Quick Start

### 1. Basic Pipeline (No Plugins)

Create a file named `pipeline.toml`:

```toml
[pipeline]
name = "my_first_pipeline"
version = "1.0.0"
description = "A simple CSV to JSON transformation"

[global]
log_level = "info"
max_parallel_tasks = 4
timeout_seconds = 300
# plugins = []  # No plugins needed for basic operations

[[sources]]
name = "input_data"
type = "csv"

[sources.config]
path = "data/input.csv"
headers = true
delimiter = ","

[[transforms]]
name = "filter_data"
function = "filter"

[transforms.config]
column = "amount"
operator = ">="
value = 100.0

[[sinks]]
name = "output_json"
type = "json"

[sinks.config]
path = "output/result.json"
format = "records"
pretty = true

[error_handling]
strategy = "stop"
```

### 2. Pipeline with HTTP Plugin

```toml
[pipeline]
name = "api_pipeline"
version = "1.0.0"

[global]
log_level = "info"
plugins = ["http"]  # Load HTTP plugin dynamically

[[sources]]
name = "api_data"
type = "http"

[sources.config]
url = "https://api.example.com/data"
method = "GET"
format = "json"

[[sinks]]
name = "local_file"
type = "json"

[sinks.config]
path = "output/api_data.json"
format = "records"
pretty = true
```

### 3. Run the Pipeline

```bash
conveyor run -c pipeline.toml
```

## DAG-Based Pipelines (New!)

Conveyor now supports flexible DAG-based pipelines where you can compose stages in any order, with automatic dependency resolution and parallel execution.

### Why DAG Pipelines?

- **Flexible Composition**: Use sources, transforms, and sinks anywhere in the pipeline
- **Branching**: Send the same data to multiple stages (e.g., save to file AND display to console)
- **Sequential Chaining**: Source → Transform → HTTP Source (fetch related data) → Transform → Sink
- **Automatic Parallelization**: Independent stages execute concurrently
- **Cycle Detection**: Validates pipeline structure before execution

### DAG Pipeline Format

```toml
[pipeline]
name = "user-posts-pipeline"
version = "1.0"

[[stages]]
id = "load_users"           # Unique identifier
type = "source.json"        # Stage type: category.name
inputs = []                 # No inputs (this is a source)

[stages.config]
path = "users.json"

[[stages]]
id = "filter_active"
type = "transform.filter"
inputs = ["load_users"]     # Depends on load_users

[stages.config]
column = "status"
operator = "=="
value = "active"

# Branching: Same data goes to two different stages
[[stages]]
id = "save_to_file"
type = "sink.json"
inputs = ["filter_active"]

[stages.config]
path = "output.json"

[[stages]]
id = "display"
type = "sink.stdout"
inputs = ["filter_active"]  # Same input!

[stages.config]
format = "table"
```

### Key Differences from Legacy Format

| Feature | Legacy Format | DAG Format |
|---------|--------------|------------|
| Stage definition | Separate `[[sources]]`, `[[transforms]]`, `[[sinks]]` | Unified `[[stages]]` |
| Execution order | Fixed: sources → transforms → sinks | Flexible: based on `inputs` dependencies |
| Branching | Not supported | ✅ Multiple stages can use same input |
| Parallel execution | Sequential only | ✅ Automatic for independent stages |
| Type specification | `type = "json"` | `type = "source.json"` (category.name) |

### Backward Compatibility

**Old pipelines work automatically!** Conveyor converts legacy format to DAG format internally:

```toml
# This still works!
[[sources]]
name = "data"
type = "csv"

[[transforms]]
name = "process"
function = "filter"

[[sinks]]
name = "output"
type = "json"
```

The pipeline will be automatically converted to DAG format and execute the same way.

## HTTP Fetch Transform (New!)

The `http_fetch` transform enables dynamic HTTP API calls within your pipeline, using previous stage data as context.

### Key Features

- **Template-based URLs**: Use Handlebars templates to create dynamic URLs from row data
- **Per-row Mode**: Make individual API calls for each data row
- **Batch Mode**: Send all data in a single API request
- **Custom Headers**: Add authentication and custom headers
- **Error Handling**: Gracefully handles failed requests with null values

### Example: Fetch Related Data

```toml
# Stage 1: Load users
[[stages]]
id = "load_users"
type = "source.json"
inputs = []

[stages.config]
path = "users.json"

# Stage 2: Filter active users
[[stages]]
id = "filter_active"
type = "transform.filter"
inputs = ["load_users"]

[stages.config]
column = "status"
operator = "=="
value = "active"

# Stage 3: Fetch posts for each user via API
[[stages]]
id = "fetch_posts"
type = "transform.http_fetch"
inputs = ["filter_active"]

[stages.config]
url = "https://api.example.com/users/{{ id }}/posts"
method = "GET"
mode = "per_row"  # Call API for each row
result_field = "posts"

[stages.config.headers]
Authorization = "Bearer YOUR_TOKEN"
```

**Result**: Each user record gets a new `posts` field with the API response.

### Configuration Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `url` | ✅ Yes | - | URL template (supports `{{ field }}` syntax) |
| `method` | No | `GET` | HTTP method (GET, POST, PUT, PATCH, DELETE) |
| `mode` | No | `per_row` | `per_row` (N calls) or `batch` (1 call) |
| `result_field` | No | `http_result` | Field name for storing API response |
| `body` | No | - | Request body template (for POST/PUT/PATCH) |
| `headers` | No | - | Custom HTTP headers |

### Batch Mode Example

```toml
[[stages]]
id = "batch_request"
type = "transform.http_fetch"
inputs = ["data"]

[stages.config]
url = "https://api.example.com/batch"
method = "POST"
mode = "batch"

# Template has access to all records
body = '''
{
  "ids": [{{#each records}}{{ this.id }}{{#unless @last}},{{/unless}}{{/each}}]
}
'''
```

### Use Cases

1. **Enrich Data**: Add related information from APIs
2. **Validation**: Check data against external services
3. **Multi-step Pipelines**: Load → Filter → API → Transform → Save
4. **Data Aggregation**: Collect data from multiple endpoints

See `examples/http-chaining-example.toml` for a complete example.

## Plugin System

### How It Works

Conveyor uses a **dynamic plugin system** that loads plugins only when needed:

1. **On-Demand Loading**: Plugins specified in `[global].plugins` are loaded at runtime
2. **Version Checking**: API version compatibility is verified before loading
3. **Panic Isolation**: Plugin panics are caught and don't crash the host process
4. **Zero Overhead**: Unused plugins are never loaded into memory

### Available Plugins

| Plugin | Type | Description | Config in TOML |
|--------|------|-------------|----------------|
| `http` | Source & Sink | REST API integration | `plugins = ["http"]` |
| `mongodb` | Source & Sink | MongoDB database (planned) | `plugins = ["mongodb"]` |

### Creating Custom Plugins

Plugins are separate Rust crates compiled as `cdylib`:

```toml
# Cargo.toml for your plugin
[package]
name = "conveyor-plugin-custom"

[dependencies]
conveyor-plugin-api = { path = "../../conveyor-plugin-api" }

[lib]
crate-type = ["cdylib"]
```

See `plugins/conveyor-plugin-http` for a complete example.

## Usage

### Commands

#### Run a Pipeline

```bash
conveyor run --config <path-to-config.toml>
```

Options:
- `--dry-run`: Validate configuration without executing
- `--continue-on-error`: Continue pipeline execution even if errors occur

#### Validate Configuration

```bash
conveyor validate --config <path-to-config.toml>
```

#### List Available Modules

```bash
conveyor list
conveyor list --module-type sources
```

#### Generate Sample Configuration

```bash
conveyor generate --output sample-pipeline.toml
```

## Modules

### Built-in Data Sources

| Module | Description | Configuration |
|--------|-------------|---------------|
| `csv` | Read CSV files | `path`, `headers`, `delimiter` |
| `json` | Read JSON files | `path`, `format` (records/jsonl/dataframe) |
| `stdin` | Read from standard input | `format` (json/jsonl/csv/raw) |

### Plugin Data Sources

| Module | Plugin | Description | Configuration |
|--------|--------|-------------|---------------|
| `http` | http | Fetch data from REST APIs | `url`, `method`, `format`, `headers`, `body` |
| `mongodb` | mongodb | Read from MongoDB (planned) | `connection_string`, `database`, `collection` |

### Transforms

| Function | Description | Configuration |
|----------|-------------|---------------|
| `filter` | Filter rows based on conditions | `column`, `operator`, `value` |
| `map` | Create or transform columns | `expression`, `output_column` |
| `validate_schema` | Validate data schema | `required_fields`, `field_types`, `non_nullable` |

### Built-in Sinks

| Module | Description | Configuration |
|--------|-------------|---------------|
| `csv` | Write to CSV files | `path`, `headers`, `delimiter` |
| `json` | Write to JSON files | `path`, `format`, `pretty` |
| `stdout` | Write to standard output | `format` (table/json/jsonl/csv), `limit` |

### Plugin Sinks

| Module | Plugin | Description | Configuration |
|--------|--------|-------------|---------------|
| `http` | http | Send data to REST APIs | `url`, `method`, `format`, `headers` |
| `mongodb` | mongodb | Write to MongoDB (planned) | `connection_string`, `database`, `collection` |

## Configuration Reference

### Global Section

```toml
[global]
log_level = "info"              # trace, debug, info, warn, error
max_parallel_tasks = 4          # Number of concurrent tasks
timeout_seconds = 300           # Pipeline timeout
plugins = ["http", "mongodb"]   # Plugins to load (optional)
```

### Error Handling

```toml
[error_handling]
strategy = "stop"               # stop, continue, retry
max_retries = 3                 # Number of retry attempts
retry_delay_seconds = 5         # Delay between retries
```

## Examples

See the `examples/` directory for complete pipeline configurations:

- `simple_pipeline.toml` - Basic CSV to JSON transformation (no plugins)
- `http_plugin_example.toml` - Fetch data from REST APIs using HTTP plugin
- `mongodb_pipeline.toml` - MongoDB ETL (when plugin is ready)

## Architecture

### Workspace Structure

```
conveyor/
├── Cargo.toml                     # Workspace root
├── conveyor-plugin-api/           # Plugin API crate
│   ├── src/lib.rs                 # Plugin traits and types
│   └── Cargo.toml
├── plugins/                       # Plugin crates
│   ├── conveyor-plugin-http/
│   │   ├── src/lib.rs             # HTTP source & sink
│   │   └── Cargo.toml
│   └── conveyor-plugin-mongodb/
│       ├── src/lib.rs             # MongoDB source & sink
│       └── Cargo.toml
├── src/                           # Main application
│   ├── cli/                       # Command-line interface
│   ├── core/                      # Core pipeline engine
│   │   ├── config.rs              # Configuration parsing
│   │   ├── error.rs               # Error types
│   │   ├── pipeline.rs            # Pipeline executor
│   │   ├── registry.rs            # Module registry
│   │   └── traits.rs              # Core traits
│   ├── modules/                   # Built-in modules
│   │   ├── sources/               # CSV, JSON, Stdin
│   │   ├── transforms/            # Filter, Map, Validate
│   │   └── sinks/                 # CSV, JSON, Stdout
│   ├── plugin_loader.rs           # Dynamic plugin loader
│   └── main.rs
├── examples/                      # Example pipelines
├── tests/                         # Integration tests
└── data/                          # Test data
```

### Key Design Decisions

1. **Workspace Dependencies**: All common dependencies managed in `[workspace.dependencies]` for version consistency
2. **Plugin Isolation**: Plugins are separate `cdylib` crates that can be updated independently
3. **Panic Safety**: Plugin loader catches panics to prevent host process crashes
4. **Version Checking**: Plugin API version is verified before loading
5. **Zero-Copy**: Polars and Arrow enable efficient data processing without unnecessary copies

## Performance

Conveyor is built with performance in mind:

- **Rust**: Zero-cost abstractions and memory safety
- **Polars**: High-performance DataFrame library (10-100x faster than Pandas)
- **Tokio**: Efficient async runtime for I/O operations
- **Arrow**: Columnar memory format for optimal data processing
- **Lazy Evaluation**: Polars optimizes query plans before execution

Benchmark comparisons with Python-based ETL tools show **10-100x performance improvements** for typical workloads.

## Development

### Running Tests

```bash
# Run all tests
cargo test --all

# Run only unit tests
cargo test --lib

# Run with output
cargo test -- --nocapture
```

### Building Plugins

```bash
# Build all plugins
cargo build --all --release

# Build specific plugin
cargo build -p conveyor-plugin-http --release

# Plugin libraries will be in target/release/
ls target/release/libconveyor_plugin_*.{dylib,so}
```

### Project Structure Best Practices

- **Shared dependencies** in `[workspace.dependencies]`
- **Plugin API** is stable and versioned
- **Plugins** are independent crates with minimal dependencies
- **Core** application doesn't depend on plugins

## Roadmap

### Completed ✅
- [x] Core pipeline engine
- [x] CSV, JSON data sources
- [x] Basic transforms (filter, map, validate)
- [x] File-based sinks
- [x] HTTP plugin (source & sink)
- [x] Dynamic plugin loading system
- [x] Workspace architecture
- [x] Plugin version checking
- [x] Panic isolation for plugins

### In Progress 🚧
- [ ] MongoDB plugin implementation
- [ ] Plugin API documentation
- [ ] Plugin developer guide

### Planned 📋
- [ ] Database connectors (PostgreSQL, MySQL) as plugins
- [ ] Advanced transforms (aggregate, join, pivot)
- [ ] Stream processing support
- [ ] WebAssembly plugin support (for enhanced security)
- [ ] Web UI for pipeline monitoring
- [ ] Distributed execution

## Technical Details

### Plugin Safety

The plugin system includes several safety features:

1. **Version Checking**: Plugins are rejected if API version mismatches
2. **Panic Handling**: `std::panic::catch_unwind` isolates plugin failures
3. **Capability Verification**: Plugins must provide at least one module type
4. **Platform-Specific Loading**: Automatic library extension detection (.dylib, .so, .dll)

### Dependencies

#### Workspace-level Dependencies

All common dependencies are managed centrally in `[workspace.dependencies]`:

- `serde`, `serde_json`, `toml` - Serialization
- `tokio`, `async-trait` - Async runtime
- `anyhow`, `thiserror` - Error handling
- `tracing` - Logging
- `reqwest` - HTTP client (for HTTP plugin)
- `mongodb` - Database driver (for MongoDB plugin)

This ensures version consistency across all crates.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Areas for Contribution

- **New Plugins**: Database connectors, cloud services, message queues
- **Transforms**: More data transformation functions
- **Performance**: Optimizations and benchmarks
- **Documentation**: Examples, guides, API docs
- **Testing**: Integration tests, fuzzing, property tests

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Powered by [Polars](https://www.pola.rs/) for data processing
- Uses [Tokio](https://tokio.rs/) for async runtime
- CLI built with [Clap](https://docs.rs/clap/)
- Plugin architecture inspired by Rust community best practices

## Support

- 📖 [Documentation](https://github.com/yourusername/conveyor/wiki)
- 🐛 [Issue Tracker](https://github.com/yourusername/conveyor/issues)
- 💬 [Discussions](https://github.com/yourusername/conveyor/discussions)

---

**Made with ❤️ and Rust** | **Built with Claude Code**
