# Conveyor Documentation

Complete documentation for Conveyor - a high-performance, TOML-based ETL CLI tool built in Rust.

## Quick Links

- 🏠 [Project Home](../README.md)
- 🚀 [Quick Start](../README.md#quick-start)
- 💻 [Installation](../README.md#installation)

## User Documentation

Documentation for using Conveyor:

### Core Concepts

- **[Configuration Reference](configuration.md)** - Complete TOML configuration guide
  - Pipeline structure and metadata
  - Global configuration options
  - Stage configuration
  - Error handling strategies
  - Environment variables

- **[DAG Pipelines](dag-pipelines.md)** - Pipeline composition and execution
  - DAG-based execution model
  - Stage types and dependencies
  - Branching patterns (fan-out, fan-in)
  - Level-based parallelism
  - Validation and error handling

### CLI Usage

- **[CLI Reference](cli-reference.md)** - Complete command-line interface guide
  - `run` - Execute pipelines
  - `validate` - Validate configurations
  - `list` - Discover available modules
  - `info` - Get detailed module information
  - `build` - Interactive stage builder
  - `stage` - Stage management commands

### Built-in Modules

- **[Modules Reference](modules-reference.md)** - All built-in sources, transforms, and sinks
  - Data Sources (CSV, JSON, stdin, file watching)
  - Transforms (filter, map, select, group by, sort, AI generation, etc.)
  - Sinks (CSV, JSON, stdout)
  - Configuration options for each module
  - Usage examples

### Advanced Features

- **[Metadata System](metadata-system.md)** - Self-documenting features
  - StageMetadata structure
  - Parameter definitions and validation
  - CLI integration (list, info, build commands)
  - Creating metadata for custom stages

- **[HTTP Fetch Transform](http-fetch-transform.md)** - Dynamic API calls within pipelines
  - Per-row mode vs. batch mode
  - Template-based URLs with Handlebars
  - Authentication and custom headers
  - Error handling

## Developer Documentation

Documentation for contributing to and extending Conveyor:

### Getting Started

- **[Development Guide](development.md)** - Building, testing, and contributing
  - Prerequisites and setup
  - Project structure
  - Development workflow
  - Building plugins
  - Code standards (formatting, linting, documentation)
  - Testing strategy
  - Debugging tips

### Plugin Development

- **[Plugin System](plugin-system.md)** - Create and use plugins
  - FFI plugins (performance-critical)
  - WASM plugins (cross-platform, sandboxed)
  - Plugin safety features
  - Creating custom plugins
  - Troubleshooting

### Architecture

- **[CLAUDE.md](../CLAUDE.md)** - Technical implementation notes
  - Architecture overview and design decisions
  - Workspace structure
  - Core components
  - Implementation challenges and solutions
  - Lessons learned
  - Performance considerations

## Documentation Organization

```
docs/
├── README.md                    # This file - documentation index
├── cli-reference.md             # CLI commands and usage
├── configuration.md             # TOML configuration reference
├── dag-pipelines.md             # DAG execution model
├── development.md               # Building and contributing
├── http-fetch-transform.md      # HTTP fetch feature guide
├── metadata-system.md           # Self-documenting system
├── modules-reference.md         # Built-in modules
└── plugin-system.md             # Plugin development
```

## Examples

The [examples/](../examples/) directory contains complete pipeline examples:

- CSV to JSON transformation
- HTTP API integration
- Data enrichment with API calls
- Real-time stream processing
- Windowing and aggregation
- AI-powered data processing

## Support

- 📖 Browse documentation above
- 🐛 [Report issues](https://github.com/yoonhoGo/conveyor/issues)
- 💬 [Join discussions](https://github.com/yoonhoGo/conveyor/discussions)

---

**Tip**: Use `conveyor --help` and `conveyor <command> --help` for built-in help, or explore available modules with `conveyor list` and `conveyor info <function>`.
