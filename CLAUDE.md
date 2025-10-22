# Conveyor - Development Notes (Claude Code)

This document provides technical details about the Conveyor project's architecture, implementation decisions, and development process for AI agents and developers working on the codebase.

> **Document Purpose**: This file is specifically for AI agents (like Claude Code) and developers to understand architectural decisions, design patterns, and implementation challenges. It focuses on **WHY** and **HOW** decisions were made, not **WHAT** features exist.

## Document Guidelines

### ✅ SHOULD Include

**Architecture & Design Decisions**
- Why we chose specific technologies or patterns
- Trade-offs considered and rationale for choices
- Design patterns and their benefits
- System boundaries and responsibilities

**Implementation Challenges**
- Real problems encountered during development
- Creative solutions and workarounds
- Failed approaches and why they didn't work
- Critical discoveries (e.g., `rstr!` macro for FFI)

**Lessons Learned**
- Insights gained from experience
- Best practices discovered
- Anti-patterns to avoid
- Performance considerations and optimizations

**Quick Reference & Navigation**
- References to detailed documentation (@docs/filename.md)
- High-level component overview
- Where to find specific information

### ❌ SHOULD NOT Include

**Detailed Usage Instructions**
- Step-by-step user guides → @docs/cli-reference.md
- Configuration examples → @docs/configuration.md
- Module parameters → @docs/modules-reference.md

**Implementation Details**
- Specific code examples (unless illustrating a challenge)
- API documentation → Generated from code
- Complete function signatures → See source code

**Frequently Changing Information**
- Specific test counts (becomes outdated quickly)
- Version-specific details
- Temporary workarounds

**Tool Usage Basics**
- How to use `cargo`, `git`, etc. → @docs/development.md
- Logging syntax → See source code
- Standard Rust practices → External Rust documentation

### 🎯 Decision Criteria

**Ask yourself**:
1. "Would this help an AI agent understand **why** the code is structured this way?"
2. "Is this information about a **solved problem** or **design decision**?"
3. "Would this be outdated if implementation details change?"

**If YES to 1 or 2**: Include it in CLAUDE.md
**If YES to 3**: Put it in specific docs or remove it

---

## Table of Contents

- [Project Overview](#project-overview)
- [Architecture](#architecture)
- [Implementation Challenges & Solutions](#implementation-challenges--solutions)
- [Lessons Learned](#lessons-learned)
- [Documentation References](#documentation-references)

## Project Overview

Conveyor is a high-performance, TOML-based ETL (Extract, Transform, Load) CLI tool built in Rust featuring:

- **📋 TOML Configuration**: Simple, declarative pipeline definitions
- **🔀 DAG-Based Pipelines**: Flexible stage composition with automatic dependency resolution
- **⚡ High Performance**: Built with Rust and Polars (10-100x faster than Python)
- **🔌 Dual Plugin System**: FFI plugins for performance, WASM plugins for security/portability
- **🔄 Async Processing**: Built on Tokio for efficient concurrent operations
- **🌊 Stream Processing**: Real-time data processing with micro-batching and windowing
- **🤖 AI Integration**: LLM-powered transforms with multiple provider support
- **📊 Self-Documenting**: Metadata system enables CLI discovery and guided stage creation

## Architecture

### Workspace Structure

Conveyor uses a Cargo workspace to organize the codebase into separate, independently compilable crates:

```
conveyor/
├── Cargo.toml                     # Workspace root with shared dependencies
├── conveyor-plugin-api/           # FFI Plugin API crate (lib)
│   ├── src/
│   │   ├── lib.rs                 # FFI-safe plugin traits and types
│   │   ├── data.rs                # FfiDataFormat
│   │   └── traits.rs              # FfiDataSource, FfiTransform, FfiSink
│   └── Cargo.toml
├── conveyor-wasm-plugin-api/      # WASM Plugin API crate (lib)
│   ├── wit/
│   │   └── conveyor-plugin.wit    # WIT interface definition
│   ├── src/lib.rs                 # Guest-side helpers and types
│   └── Cargo.toml
├── plugins/                       # FFI Plugin crates (cdylib)
│   ├── conveyor-plugin-http/
│   │   ├── src/lib.rs             # HTTP source & sink (FFI)
│   │   └── Cargo.toml
│   ├── conveyor-plugin-mongodb/
│   │   ├── src/lib.rs             # MongoDB source & sink (FFI)
│   │   └── Cargo.toml
│   └── conveyor-plugin-test/
│       ├── src/lib.rs             # Test plugin (FFI)
│       └── Cargo.toml
├── plugins-wasm/                  # WASM Plugin crates (cdylib → .wasm)
│   └── conveyor-plugin-echo-wasm/
│       ├── src/lib.rs             # Echo plugin (WASM)
│       └── Cargo.toml
├── examples/
│   └── plugin-template/           # FFI plugin template
├── tests/
│   ├── integration_test.rs        # FFI plugin tests
│   └── wasm_plugin_test.rs        # WASM plugin tests
└── src/                           # Main application
    ├── main.rs
    ├── core/                      # Core pipeline engine
    ├── modules/                   # Built-in modules
    ├── plugin_loader.rs           # FFI plugin loader
    └── wasm_plugin_loader.rs      # WASM plugin loader
```

**Key Design Decisions**:

1. **Workspace Dependencies**: All common dependencies (serde, tokio, anyhow, etc.) are defined in `[workspace.dependencies]` for version consistency across all crates
2. **Plugin Isolation**: Plugins are separate `cdylib` crates compiled as dynamic libraries, not linked into the main binary
3. **Independent Compilation**: Each plugin can be built separately with `cargo build -p plugin-name`
4. **Shared API Crate**: Plugin API crates provide common types and traits used by both host and plugins

### Core Components

For detailed information about each component, see:

- **Configuration System** → @docs/configuration.md
- **DAG Pipeline Executor** → @docs/dag-pipelines.md
- **Module Registry & Stages** → @docs/modules-reference.md
- **CLI Commands** → @docs/cli-reference.md
- **Metadata System** → @docs/metadata-system.md
- **Plugin System** → @docs/plugin-system.md
- **Development Workflow** → @docs/development.md

#### Quick Component Overview

1. **Configuration System** (`core/config.rs`)
   - TOML deserialization with strong typing
   - DAG-based pipeline configuration
   - Plugin loading specification
   - Global variables with environment substitution

2. **Stage System** (`core/stage.rs`, `core/traits.rs`)
   - Unified `Stage` trait for all modules
   - Function-based naming: "csv.read", "filter.apply", "json.write"
   - HashMap-based input/output passing
   - Async execution with `async_trait`

3. **Data Format** (`core/traits.rs`)
   ```rust
   pub enum DataFormat {
       DataFrame(DataFrame),      // Polars DataFrame for structured data
       RecordBatch(RecordBatch),  // Vec of HashMaps for flexible JSON-like data
       Raw(Vec<u8>),              // Raw bytes for binary data
       Stream(Stream),            // Async stream for real-time processing
   }
   ```

4. **DAG Pipeline Executor** (`core/dag_executor.rs`)
   - Topological ordering for dependency resolution
   - Level-based parallelism for independent stages
   - Cycle detection during validation
   - Error recovery strategies (stop/continue/retry)

5. **Module Registry** (`core/registry.rs`)
   - Function-based API: `register_function()`, `get_function()`, `list_functions()`
   - Dynamic registration for plugin modules
   - Simplified from legacy 26 methods to 6 core methods

## Implementation Challenges & Solutions

### 1. Async Trait Methods

**Challenge**: Rust doesn't natively support async methods in traits.

**Solution**: Used `async_trait` crate for clean async trait syntax:
```rust
#[async_trait]
pub trait Stage: Send + Sync {
    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat>;
}
```

### 2. Type Safety in Dynamic Configuration

**Challenge**: TOML values are dynamic (`toml::Value`), but we need type safety.

**Solution**: Pattern matching with helpful error messages:
```rust
let value = config
    .get("key")
    .and_then(|v| v.as_str())
    .ok_or_else(|| anyhow::anyhow!("Missing required 'key' configuration"))?;
```

### 3. Dynamic Plugin Loading and FFI Safety

**Challenge**: Creating an FFI-safe plugin interface that works across Rust compiler versions.

**Successful Solution**: Full FFI implementation using `abi_stable` crate 0.11

1. **abi_stable crate**: Provides ABI-stable types for cross-compiler compatibility
   - `#[sabi_trait]` macro for FFI-safe trait objects
   - `StableAbi` derive macro for FFI-safe types
   - FFI-safe types: `RString`, `RVec`, `RResult`, `RBoxError`, `RHashMap`
   - `#[repr(C)]` for C-compatible memory layout
   - Works perfectly with complex enums and traits

2. **FFI-Safe Traits**: Successfully implemented three core traits
   - `FfiDataSource`: Read data from sources
   - `FfiTransform`: Transform data
   - `FfiSink`: Write data to destinations
   - All use `#[sabi_trait]` macro for automatic FFI safety

3. **FFI-Safe Data Format**: `FfiDataFormat` enum with three variants
   - `ArrowIpc(RVec<u8>)`: Polars DataFrame as Arrow IPC
   - `JsonRecords(RVec<u8>)`: JSON serialized records
   - `Raw(RVec<u8>)`: Raw bytes
   - Uses `#[repr(C)]` and `#[derive(StableAbi)]`

4. **Plugin Declaration**: Static symbol pattern with critical `rstr!` macro
   ```rust
   #[no_mangle]
   pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
       api_version: PLUGIN_API_VERSION,
       name: rstr!("plugin_name"),  // Must use rstr! for const init
       version: rstr!("0.1.0"),
       description: rstr!("Plugin description"),
       register,
   };
   ```

5. **Dynamic Library Loading**: Uses `libloading` with safety features
   - Panic isolation with `std::panic::catch_unwind`
   - API version checking before plugin activation
   - Platform-specific library loading (.dylib, .so, .dll)

**Key Discovery**:
- Initial attempts with `RStr::from()` and `RStr::from_str()` failed in static context
- **Correct solution**: Use `rstr!("literal")` macro for const RStr initialization
- This was the critical missing piece for successful FFI implementation

**Lessons Learned**:
- `abi_stable` successfully handles complex Rust types across FFI boundaries
- `#[sabi_trait]` makes trait objects FFI-safe automatically
- `rstr!` macro is essential for static string initialization
- FFI in Rust is achievable with the right tools and patterns
- Proper documentation and examples are crucial for plugin developers

**Plugin Implementations**:
- **HTTP Plugin**: REST API source and sink (GET/POST/PUT/PATCH/DELETE) → @docs/plugins/http.md
- **MongoDB Plugin**: Cursor-based pagination, batch inserts, connection pooling → @docs/plugins/mongodb.md

### 4. WASM Plugin System

**Challenge**: Provide secure, cross-platform plugin execution.

**Solution**: WebAssembly Component Model with WASI Preview 2

1. **Architecture**:
   - Uses wasmtime runtime with WASI Preview 2
   - Complete memory isolation from host
   - WIT (WebAssembly Interface Types) definitions
   - Capability-based security with ResourceTable

2. **File System Access Challenge**:
   WASM sandbox blocked file I/O with error:
   ```
   failed to find a pre-opened file descriptor
   ```

   **Solution**: Research via web search revealed WASI `preopened_dir` API:
   ```rust
   let wasi = WasiCtxBuilder::new()
       .inherit_stdio()
       .preopened_dir(current_dir_str, ".", DirPerms::all(), FilePerms::all())?
       .build();
   ```

3. **Data Exchange**:
   - DataFrame → JSON records → WASM
   - WASM → JSON records → DataFrame
   - Custom `anyvalue_to_json()` for comprehensive Polars type mapping

4. **Performance Characteristics**:
   - 5-15% overhead vs FFI (due to JSON serialization)
   - Memory-safe by design
   - Cross-platform: same .wasm works everywhere
   - No Rust compiler version requirements

**WASM Plugin Example**:
- **Echo Plugin**: Test plugin for WASM system validation

### 5. Workspace Dependency Management

**Challenge**: Managing dependency versions across multiple crates (main binary, plugin APIs, plugins).

**Solution**: Centralized workspace dependencies:
```toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.47", features = ["full"] }
polars = { version = "0.45", features = ["json", "csv"] }

[dependencies]
serde = { workspace = true }  # Use workspace version
tokio = { workspace = true }
```

**Benefits**:
- Single source of truth for versions
- Easier dependency updates
- Consistent feature flags across crates
- Prevents version conflicts

## Lessons Learned

### Core Rust Principles

1. **Type Safety First**: Strong typing catches errors at compile time
   - `toml::Value` → strongly typed config structs
   - Compiler-enforced trait implementations
   - Enum exhaustiveness checking

2. **Async Everywhere**: Consistent async makes composition easier
   - Use `#[async_trait]` for trait methods
   - Avoid nested tokio runtimes
   - async/await throughout the call chain

3. **Unified Architecture**: Stage-based design enables simplicity
   - Single `Stage` trait replaces DataSource/Transform/Sink
   - Function-based API: "csv.read", "filter.apply", "json.write"
   - Simplified registry (26 methods → 6 methods)
   - Plugin system built on dynamic Stage loading

### Plugin System Insights

4. **Dynamic Plugin Loading**: Significant architectural benefits
   - Reduced binary size (plugins not compiled in)
   - Zero overhead for unused features
   - Independent plugin development and versioning
   - User chooses which plugins to load

5. **FFI Success with `abi_stable`**: ⭐ SOLVED
   - `abi_stable` crate enables true FFI-safe Rust-to-Rust plugins
   - `#[sabi_trait]` macro creates FFI-safe trait objects automatically
   - `rstr!` macro for const RStr initialization in static context
   - `#[repr(C)]` + `StableAbi` derive for FFI-safe types
   - Key discovery: Use `rstr!("literal")` macro in static declarations

6. **Panic Isolation is Critical**: Plugins can crash without affecting host
   - `std::panic::catch_unwind` is essential for plugin systems
   - Prevents cascade failures
   - Better error messages for debugging

7. **Version Checking**: API versioning prevents incompatible plugins
   - Simple version constant: `PLUGIN_API_VERSION`
   - Reject incompatible plugins early
   - Clear error messages about version mismatches

8. **WASM for Security and Portability**:
   - Complete memory isolation (sandboxed execution)
   - Cross-platform compatibility (write once, run anywhere)
   - File system access requires explicit permissions (preopened_dir)
   - JSON serialization adds 5-15% overhead but ensures safety

### Workspace Management

9. **Workspace Dependencies**: Centralized version management
   - `[workspace.dependencies]` eliminates version conflicts
   - Single source of truth for dependency versions
   - Consistent feature flags across crates
   - Easier to update and maintain

10. **Crate Organization**: Separation of concerns
    - Plugin API as separate crate
    - Each plugin as independent crate with `cdylib`
    - Main binary doesn't depend on plugins
    - Plugins depend only on plugin API

### Development Practices

11. **Test Early**: Tests help validate API compatibility
    - Polars API changes caught by tests
    - Integration tests validate end-to-end flows
    - Unit tests for each module

12. **Performance Matters**: Polars' performance is a key differentiator
    - 10-100x faster than Python alternatives
    - Zero-copy operations with Arrow
    - Lazy evaluation for query optimization

13. **Documentation as Development Tool**: Writing docs clarifies design
    - README.md for users
    - CLAUDE.md for technical details (this file)
    - Detailed reference docs for each feature
    - Helps identify inconsistencies and gaps

14. **Metadata System for Self-Documentation**:
    - Embed documentation in code (StageMetadata)
    - Enable CLI discovery and validation
    - Interactive builder for guided stage creation
    - Single source of truth for all documentation

### Development Workflow

15. **Debug vs Release Builds**:
    - Use debug mode during active development for faster iteration
    - Always run release build before completing work
    - Release build catches optimization-related issues
    - Workflow:
      ```bash
      # During development (fast iteration)
      cargo build
      cargo run -- run pipeline.toml

      # Before completion (verify release compatibility)
      cargo build --release
      cargo run --release -- run pipeline.toml
      ```

16. **Code Quality Checks**:
    - Always run format and lint before committing
    - Format ensures consistent code style
    - Clippy catches common mistakes and suggests improvements
    - Standard workflow:
      ```bash
      # Format code
      cargo fmt --all

      # Run linter
      cargo clippy --all-targets --all-features -- -D warnings

      # Run tests
      cargo test --all
      ```
    - Consider using pre-commit hooks for automation

17. **Release Process**:
    - When a release is requested, update version numbers and create a git tag
    - Process:
      1. Update version in `Cargo.toml` (workspace root and all crates)
      2. Run `cargo build` to update `Cargo.lock`
      3. Commit changes with message: "chore: bump version to X.Y.Z"
      4. Create git tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z"`
      5. **DO NOT push** - leave push decision to the user
    - Example workflow:
      ```bash
      # Update version in Cargo.toml files
      # Run build to update Cargo.lock
      cargo build

      # Commit version bump
      git add Cargo.toml Cargo.lock
      git commit -m "chore: bump version to 0.9.0"

      # Create annotated tag
      git tag -a v0.9.0 -m "Release v0.9.0"

      # User decides when to push
      # git push && git push --tags
      ```
    - Rationale: User should control when releases are published to remote

### Plugin Systems Comparison

**Use FFI plugins when:**
- Performance is critical (near-zero overhead)
- Direct memory access needed
- Native async/await required
- Same platform/compiler version acceptable

**Use WASM plugins when:**
- Cross-platform compatibility required
- Sandboxed execution needed
- Untrusted code execution
- Independent compiler versions
- 5-15% overhead acceptable

## Documentation References

This document focuses on architecture decisions and implementation challenges. For detailed information about specific features:

### User Documentation
- **[@docs/cli-reference.md](docs/cli-reference.md)** - Complete CLI command reference
- **[@docs/configuration.md](docs/configuration.md)** - Pipeline configuration guide
- **[@docs/dag-pipelines.md](docs/dag-pipelines.md)** - DAG pipeline composition and execution
- **[@docs/builtin-functions.md](docs/builtin-functions.md)** - Built-in sources, transforms, and sinks usage
- **[@docs/plugins/http.md](docs/plugins/http.md)** - HTTP plugin usage and configuration
- **[@docs/plugins/mongodb.md](docs/plugins/mongodb.md)** - MongoDB plugin usage and configuration
- **[@docs/http-fetch-transform.md](docs/http-fetch-transform.md)** - Dynamic HTTP API calls within pipelines
- **[@docs/modules-reference.md](docs/modules-reference.md)** - Index of all available modules

### Developer Documentation
- **[@docs/plugin-system.md](docs/plugin-system.md)** - Creating custom plugins (FFI & WASM)
- **[@docs/metadata-system.md](docs/metadata-system.md)** - Self-documenting system for developers
- **[@docs/development.md](docs/development.md)** - Building, testing, and contributing
- **[README.md](README.md)** - Project overview and quick start for users

### Testing Strategy

All modules include comprehensive unit tests using `#[cfg(test)]`. Tests follow these key patterns:

1. **Positive tests**: Valid configurations and operations
2. **Negative tests**: Invalid configs, missing fields
3. **Edge cases**: Empty data, large files, special characters
4. **Plugin lifecycle**: Load, execute, unload
5. **Error recovery**: Graceful handling of plugin failures

See @docs/development.md for detailed testing workflow and best practices.

## Performance Considerations

Conveyor achieves high performance through:

1. **Zero-Copy Operations**: Polars uses Apache Arrow's columnar format for efficient data access
2. **Lazy Evaluation**: Query optimization through lazy execution plans
3. **Async I/O**: Non-blocking operations with Tokio runtime
4. **Memory Efficiency**: Streaming support, in-place operations, on-demand plugin loading

For implementation details, see the codebase and @docs/development.md.

---

**Note**: This document is intended for AI agents (like Claude Code) and developers working on the codebase. For user-facing documentation, see the docs referenced above.
