# Conveyor Documentation

Complete documentation for Conveyor - a high-performance, TOML-based ETL CLI tool built in Rust.

## Quick Links

- 🏠 [Project Home](../README.md)
- 🚀 [Quick Start](../README.md#quick-start)
- 💻 [Installation](../README.md#installation)

## User Documentation

Learn how to use Conveyor for your data pipelines.

### Getting Started

- **[CLI Reference](cli-reference.md)** - All CLI commands and usage
  - `run`, `validate`, `list`, `info`, `build`, `stage`
  - Global options and environment variables
  - Interactive features and discovery workflow

- **[Configuration Guide](configuration.md)** - TOML pipeline configuration
  - Pipeline structure and metadata
  - Global configuration and variables
  - Stage configuration
  - Error handling strategies

- **[DAG Pipelines](dag-pipelines.md)** - Pipeline composition and execution
  - DAG-based execution model
  - Branching patterns (fan-out, fan-in)
  - Level-based parallelism
  - Data passing between stages

### Functions and Plugins

- **[Modules Reference](modules-reference.md)** - Index of all available functions
  - Quick navigation to all sources, transforms, and sinks
  - Built-in and plugin function lists

- **[Built-in Functions](builtin-functions.md)** - Detailed documentation for built-in functions
  - Data Sources (CSV, JSON, stdin, file watching)
  - Transforms (filter, map, select, group by, sort, AI, etc.)
  - Sinks (CSV, JSON, stdout)
  - Configuration options and examples

- **[HTTP Plugin](plugins/http.md)** - REST API integration
  - GET, POST, PUT, PATCH, DELETE operations
  - Authentication and custom headers
  - Source and sink modes
  - Complete examples

- **[MongoDB Plugin](plugins/mongodb.md)** - Database operations
  - Read operations (find, findOne)
  - Write operations (insert, update, delete, replace)
  - Query language and best practices
  - Performance tips

### Advanced Features

- **[HTTP Fetch Transform](http-fetch-transform.md)** - Dynamic API calls
  - Per-row mode vs. batch mode
  - Template-based URLs with Handlebars
  - Authentication and error handling
  - Multi-step pipeline examples

## Developer Documentation

Extend Conveyor and contribute to the project.

### Plugin Development

- **[Plugin Development Guide](plugin-system.md)** - Create custom plugins
  - FFI plugins (high-performance, Rust-to-Rust)
  - WASM plugins (cross-platform, sandboxed)
  - Plugin capabilities (sources, transforms, sinks)
  - Testing and distribution

- **[Metadata System](metadata-system.md)** - Self-documenting features
  - StageMetadata structure
  - Parameter definitions and validation
  - CLI integration (list, info, build)
  - Implementing metadata for custom stages

### Contributing

- **[Development Guide](development.md)** - Build, test, contribute
  - Prerequisites and setup
  - Project structure
  - Development workflow
  - Code standards and testing
  - Release process

- **[Architecture (CLAUDE.md)](../CLAUDE.md)** - Technical implementation details
  - Architecture overview and design decisions
  - Workspace structure
  - Implementation challenges and solutions
  - Lessons learned
  - For AI agents and developers

## Documentation Structure

```
docs/
├── README.md                    # This file - documentation index
├── cli-reference.md             # CLI commands and usage
├── configuration.md             # TOML configuration reference
├── dag-pipelines.md             # DAG execution model
├── modules-reference.md         # Index of all functions
├── builtin-functions.md         # Built-in functions detail
├── plugins/                     # Plugin usage guides
│   ├── http.md
│   └── mongodb.md
├── http-fetch-transform.md      # HTTP fetch feature guide
├── metadata-system.md           # Self-documenting system
├── plugin-system.md             # Plugin development
└── development.md               # Building and contributing
```

## Examples

The [examples/](../examples/) directory contains complete pipeline examples:

1. **CSV to JSON**: Simple data transformation
2. **HTTP API Integration**: Fetch and process external data
3. **Data Enrichment**: Combine data from multiple sources
4. **Stream Processing**: Real-time data analysis with windowing
5. **AI Classification**: Sentiment analysis with LLMs

## Common Workflows

### Discover Available Functions

```bash
# List all functions
conveyor list

# Filter by type
conveyor list -t sources

# Get detailed info
conveyor info csv.read
```

### Build a Pipeline

```bash
# Create new pipeline
conveyor stage new -o pipeline.toml

# Build stages interactively
conveyor build

# Validate configuration
conveyor validate pipeline.toml

# Run pipeline
conveyor run pipeline.toml
```

### Use Plugins

```bash
# Enable plugins in pipeline
[global]
plugins = ["http", "mongodb"]

# Use plugin functions
[[stages]]
function = "http.get"

[[stages]]
function = "mongodb.find"
```

## Support

- 📖 Browse documentation above
- 🐛 [Report issues](https://github.com/yoonhoGo/conveyor/issues)
- 💬 [Join discussions](https://github.com/yoonhoGo/conveyor/discussions)

---

**Tip**: Use `conveyor --help` for built-in help, or explore with `conveyor list` and `conveyor info <function>` for interactive discovery.
