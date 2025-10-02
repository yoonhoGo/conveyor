# Conveyor CLI

> A high-performance TOML-based ETL CLI tool for data pipelines

[![npm version](https://badge.fury.io/js/%40conveyor%2Fcli.svg)](https://www.npmjs.com/package/@conveyor/cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## 🚀 Quick Start

### Installation

```bash
# Using npm
npm install -g @conveyor/cli

# Using npx (no installation required)
npx @conveyor/cli --help
```

### Usage

```bash
# Run a pipeline
conveyor run -c pipeline.toml

# List available modules
conveyor list

# Validate pipeline configuration
conveyor validate -c pipeline.toml
```

## 📋 Features

- **TOML-based Configuration**: Simple, readable pipeline definitions
- **High Performance**: Built with Rust and Polars (10-100x faster than Python)
- **Dual Plugin System**:
  - **FFI Plugins**: Maximum performance with native shared libraries
  - **WASM Plugins**: Sandboxed, cross-platform, language-agnostic
- **Built-in Modules**: CSV, JSON, stdin/stdout sources and sinks
- **Data Transformations**: Filter, map, validate, and more

## 📦 What's Included

- Built-in data sources and sinks (CSV, JSON, stdin/stdout)
- Data transformation modules (filter, map, validate)
- Plugin system for extensibility
- Async execution with configurable parallelism
- Comprehensive error handling

## 🔌 Plugins

### FFI Plugins (Native Performance)
- **HTTP Plugin**: REST API integration
- **MongoDB Plugin**: Database source and sink

### WASM Plugins (Sandboxed & Cross-platform)
- Language-agnostic plugin development
- Complete memory isolation
- Single binary runs everywhere

## 📖 Documentation

For full documentation, visit: [GitHub Repository](https://github.com/yoonhoGo/conveyor)

## 🛠️ Development

This is a Rust project distributed via npm for easy installation. The package automatically downloads platform-specific binaries or builds from source if Rust is installed.

**Supported Platforms:**
- macOS (ARM64, x64)
- Linux (x64, ARM64)
- Windows (x64)

## 📄 License

MIT © Yoonho Go

## 🤝 Contributing

Contributions are welcome! Please check out the [GitHub repository](https://github.com/yoonhoGo/conveyor) for more information.
