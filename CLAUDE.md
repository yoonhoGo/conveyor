# Conveyor - Development Notes (Claude Code)

This document provides technical details about the Conveyor project's architecture, implementation decisions, and development process. It was created as part of a conversation with Claude Code.

## Project Overview

Conveyor is a TOML-based ETL CLI tool built in Rust, designed to provide high-performance data pipeline processing with a simple, declarative configuration approach.

### Design Goals

1. **Simplicity**: TOML-based configuration that's easy to read and write
2. **Performance**: Leverage Rust and Polars for 10-100x faster processing than Python alternatives
3. **Extensibility**: Modular architecture supporting plugins and custom modules
4. **Safety**: Rust's type system ensures memory safety and prevents common bugs
5. **Production-Ready**: Comprehensive error handling, logging, and testing

## Architecture

### Workspace Structure

Conveyor uses a Cargo workspace to organize the codebase into separate, independently compilable crates:

```
conveyor/
├── Cargo.toml                     # Workspace root with shared dependencies
├── conveyor-plugin-api/           # Plugin API crate (lib)
│   ├── src/lib.rs                 # Plugin traits and types
│   └── Cargo.toml
├── plugins/                       # Plugin crates (cdylib)
│   ├── conveyor-plugin-http/
│   │   ├── src/lib.rs             # HTTP source & sink
│   │   └── Cargo.toml
│   └── conveyor-plugin-mongodb/
│       ├── src/lib.rs             # MongoDB source & sink
│       └── Cargo.toml
└── src/                           # Main application
    ├── main.rs
    ├── core/
    ├── modules/
    └── plugin_loader.rs
```

**Key Design Decisions**:

1. **Workspace Dependencies**: All common dependencies (serde, tokio, anyhow, etc.) are defined in `[workspace.dependencies]` for version consistency across all crates
2. **Plugin Isolation**: Plugins are separate `cdylib` crates compiled as dynamic libraries, not linked into the main binary
3. **Independent Compilation**: Each plugin can be built separately with `cargo build -p plugin-name`
4. **Shared API Crate**: `conveyor-plugin-api` provides common types and traits used by both host and plugins

### Core Components

#### 1. Configuration System (`core/config.rs`)

The configuration system uses `serde` for TOML deserialization with strong typing:

```rust
pub struct PipelineConfig {
    pub pipeline: PipelineMetadata,
    pub global: GlobalConfig,
    pub sources: Vec<SourceConfig>,
    pub transforms: Vec<TransformConfig>,
    pub sinks: Vec<SinkConfig>,
    pub error_handling: ErrorHandlingConfig,
}
```

Key features:
- Default values for optional fields
- Comprehensive validation
- Support for nested configurations
- Plugin loading via `GlobalConfig.plugins: Vec<String>`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub log_level: String,
    pub max_parallel_tasks: usize,
    pub timeout_seconds: u64,

    /// List of plugins to load dynamically (e.g., ["http", "mongodb"])
    #[serde(default)]
    pub plugins: Vec<String>,
}
```

#### 2. Trait System (`core/traits.rs`)

The core abstraction uses three main traits:

```rust
#[async_trait]
pub trait DataSource: Send + Sync {
    async fn name(&self) -> &str;
    async fn read(&self, config: &HashMap<String, toml::Value>) -> Result<DataFormat>;
    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()>;
}

#[async_trait]
pub trait Transform: Send + Sync {
    async fn name(&self) -> &str;
    async fn apply(&self, data: DataFormat, config: &Option<HashMap<String, toml::Value>>)
        -> Result<DataFormat>;
    async fn validate_config(&self, config: &Option<HashMap<String, toml::Value>>) -> Result<()>;
}

#[async_trait]
pub trait Sink: Send + Sync {
    async fn name(&self) -> &str;
    async fn write(&self, data: DataFormat, config: &HashMap<String, toml::Value>) -> Result<()>;
    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()>;
}
```

**Design Decision**: Using `async_trait` for async methods in traits, enabling async I/O operations throughout the pipeline.

#### 3. Data Format (`core/traits.rs`)

The `DataFormat` enum handles different data representations:

```rust
pub enum DataFormat {
    DataFrame(DataFrame),      // Polars DataFrame for structured data
    RecordBatch(RecordBatch),  // Vec of HashMaps for flexible JSON-like data
    Raw(Vec<u8>),              // Raw bytes for binary data
}
```

This abstraction allows seamless conversion between formats while maintaining performance.

#### 4. Pipeline Executor (`core/pipeline.rs`)

The pipeline executor orchestrates data flow:

```rust
pub struct Pipeline {
    config: PipelineConfig,
    registry: Arc<ModuleRegistry>,
    plugin_loader: PluginLoader,
}
```

Features:
- Sequential execution of sources → transforms → sinks
- Dynamic plugin loading on initialization
- Data passing between stages
- Timeout handling
- Error recovery based on strategy (stop/continue/retry)

**Plugin Integration**: The pipeline loads plugins specified in `config.global.plugins` during initialization:
```rust
pub async fn new(config: PipelineConfig) -> Result<Self> {
    let registry = Arc::new(ModuleRegistry::with_defaults().await?);

    // Load plugins dynamically
    let mut plugin_loader = PluginLoader::new();
    if !config.global.plugins.is_empty() {
        plugin_loader.load_plugins(&config.global.plugins)?;
    }

    Ok(Self { config, registry, plugin_loader })
}
```

#### 5. Module Registry (`core/registry.rs`)

The registry manages available modules:

```rust
pub struct ModuleRegistry {
    sources: HashMap<String, DataSourceRef>,
    transforms: HashMap<String, TransformRef>,
    sinks: HashMap<String, SinkRef>,
}
```

Supports dynamic module registration with the plugin system.

#### 6. Dynamic Plugin System (`plugin_loader.rs`)

The dynamic plugin system loads plugins at runtime as shared libraries:

```rust
pub struct PluginLoader {
    plugin_dir: PathBuf,
    plugins: HashMap<String, PluginHandle>,
}

struct PluginHandle {
    _library: Library,
    name: String,
}
```

**Architecture Overview**:

1. **On-Demand Loading**: Plugins are NOT compiled into the binary. They're loaded only when specified in TOML:
   ```toml
   [global]
   plugins = ["http", "mongodb"]  # Load these plugins at runtime
   ```

2. **Dynamic Library Loading**: Plugins are `cdylib` crates compiled to:
   - macOS: `libconveyor_plugin_*.dylib`
   - Linux: `libconveyor_plugin_*.so`
   - Windows: `conveyor_plugin_*.dll`

3. **Zero Overhead**: Unused plugins are never loaded into memory, reducing binary size and startup time

**Safety Features**:

```rust
/// Load a plugin with version checking and panic isolation
pub fn load_plugin(&mut self, name: &str) -> Result<()> {
    // Catch panics during plugin loading
    let result = std::panic::catch_unwind(|| {
        self.load_plugin_internal(&library_path, name)
    });

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(panic_err) => {
            Err(anyhow::anyhow!(
                "Plugin '{}' panicked during loading: {}",
                name, panic_msg
            ))
        }
    }
}
```

**Key Safety Features**:

1. **Panic Isolation**: `std::panic::catch_unwind` prevents plugin crashes from affecting the host process
2. **Version Checking**: Plugin API version is verified before loading (PLUGIN_API_VERSION constant)
3. **Capability Verification**: Plugins must provide at least one module type (source/sink/transform)
4. **Platform Detection**: Automatic library extension detection for cross-platform support

**Plugin Examples**:

- **HTTP Plugin** (`plugins/conveyor-plugin-http/`):
  - REST API source and sink
  - GET/POST/PUT/PATCH/DELETE methods
  - JSON, JSONL, CSV, and raw formats
  - Custom headers and timeouts
  - Only loaded when `plugins = ["http"]` is specified

- **MongoDB Plugin** (`plugins/conveyor-plugin-mongodb/`):
  - Cursor-based pagination for large datasets
  - Batch insert support
  - Connection pooling
  - Only loaded when `plugins = ["mongodb"]` is specified

### Module Implementation

#### Sources

**CSV Source** (`modules/sources/csv.rs`):
- Uses Polars' `CsvReader` with optimizations
- Supports custom delimiters and headers
- Schema inference capabilities

**JSON Source** (`modules/sources/json.rs`):
- Multiple format support (records, jsonl, dataframe)
- Handles large files efficiently
- Converts to appropriate DataFormat

**Stdin Source** (`modules/sources/stdin.rs`):
- Reads from standard input
- Format detection (json, jsonl, csv, raw)
- Enables pipeline chaining with Unix tools

#### Transforms

**Filter Transform** (`modules/transforms/filter.rs`):
- Supports comparison operators: `==`, `!=`, `>`, `>=`, `<`, `<=`
- String operations: `contains`, `in`
- Works on DataFrame columns efficiently

**Map Transform** (`modules/transforms/map.rs`):
- Simple expression evaluation
- Arithmetic operations: `+`, `-`, `*`, `/`
- Column creation and modification

**Validate Schema Transform** (`modules/transforms/validate.rs`):
- Required fields checking
- Type validation
- Null constraint enforcement
- Unique value validation

#### Sinks

**CSV Sink** (`modules/sinks/csv.rs`):
- Writes Polars DataFrame to CSV
- Custom delimiters and headers
- Automatic directory creation

**JSON Sink** (`modules/sinks/json.rs`):
- Multiple output formats
- Pretty printing option
- Efficient serialization

**Stdout Sink** (`modules/sinks/stdout.rs`):
- Table, JSON, JSONL, CSV output formats
- Row limiting for preview
- Colorized output (potential future enhancement)

## Implementation Challenges & Solutions

### 1. Polars API Changes

**Challenge**: Polars 0.44 had breaking API changes from earlier versions.

**Solutions**:
- `CsvReader::from_path()` → `CsvReader::new(file)` with `CsvReadOptions`
- `has_header()` → `with_has_header()`
- `DataType::Categorical` removed in some contexts
- Column operations require `.as_materialized_series()`

### 2. String Type Conversions

**Challenge**: Polars 0.44 uses `PlSmallStr` instead of `&str` for column names.

**Solution**: Added `.into()` conversions and `.to_string()` where needed:
```rust
Series::new("column".into(), values)  // PlSmallStr conversion
column.name().to_string()              // Get String from PlSmallStr
```

### 3. Expression API

**Challenge**: Some Expr methods like `.str().contains()` and `.is_in()` changed or were removed.

**Solutions**:
- `contains`: Implemented manual filtering with iteration
- `is_in`: Used multiple `eq()` operations combined with `or()`

### 4. Async Trait Methods

**Challenge**: Rust doesn't natively support async methods in traits.

**Solution**: Used `async_trait` crate for clean async trait syntax:
```rust
#[async_trait]
pub trait DataSource: Send + Sync {
    async fn read(&self, config: &HashMap<String, toml::Value>) -> Result<DataFormat>;
}
```

### 5. Type Safety in Dynamic Configuration

**Challenge**: TOML values are dynamic (`toml::Value`), but we need type safety.

**Solution**: Pattern matching with helpful error messages:
```rust
let value = config
    .get("key")
    .and_then(|v| v.as_str())
    .ok_or_else(|| anyhow::anyhow!("Missing required 'key' configuration"))?;
```

### 6. Dynamic Plugin Loading and FFI Safety

**Challenge**: Creating an FFI-safe plugin interface that works across Rust compiler versions.

**Attempted Solutions**:

1. **abi_stable crate**: Provides ABI-stable types for cross-compiler compatibility
   - **Issue**: Doesn't support enums with data (like `Result<T, E>` or `DataFormat`)
   - **Error**: "`#[::safer_ffi::derive_ReprC]`: Non field-less `enum`s are not supported yet"
   - **Outcome**: Too complex for our use case

2. **safer-ffi crate**: Simplified FFI safety with C-compatible types
   - **Issue**: Same limitation - can't handle complex enums
   - **Error**: Similar compilation errors with enum variants
   - **Outcome**: Insufficient for our data types

3. **Direct C FFI**: Raw `extern "C"` functions
   - **Issue**: Trait objects (`dyn Trait`) are not FFI-safe
   - **Warning**: "`extern` fn uses type `dyn Plugin`, which is not FFI-safe"
   - **Reason**: No stable ABI for trait objects across compiler versions

**Current Solution**: Simplified plugin loading without full FFI interface
- Uses `libloading` for dynamic library loading
- Focuses on panic isolation and version checking
- Plugins share the same Rust compiler version as host
- Future consideration: WebAssembly plugins for enhanced sandboxing

**Lessons Learned**:
- FFI in Rust is complex and requires careful type design
- Not all Rust types can cross FFI boundaries safely
- Workspace with shared compiler version is acceptable for many use cases
- Perfect cross-compiler compatibility may require significant API compromises

### 7. Workspace Dependency Management

**Challenge**: Managing dependency versions across multiple crates (main binary, plugin API, plugins).

**Solution**: Centralized workspace dependencies:
```toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.47", features = ["full"] }
# ... other dependencies

[dependencies]
serde = { workspace = true }  # Use workspace version
tokio = { workspace = true }
```

**Benefits**:
- Single source of truth for versions
- Easier dependency updates
- Consistent feature flags across crates
- Prevents version conflicts

## Testing Strategy

### Unit Tests

Located in each module using `#[cfg(test)]`:
- Config validation (5 tests)
- Registry operations (3 tests)
- Source modules (8 tests)
- Plugin system (12 tests)
- Total: 28+ unit tests

### Integration Tests

Located in `tests/integration_test.rs`:
- End-to-end pipeline scenarios
- File I/O with temporary directories
- Config parsing verification
- Total: 3 integration tests

### Test Coverage

```
test result: ok. 36 passed; 0 failed; 0 ignored
```

Key test patterns:
1. **Positive tests**: Valid configurations and operations
2. **Negative tests**: Invalid configs, missing fields
3. **Edge cases**: Empty data, large files, special characters

## Performance Considerations

### 1. Zero-Copy Operations

Polars uses Apache Arrow's columnar format, enabling zero-copy data access:
```rust
let column = df.column("name")?;  // No data copy
```

### 2. Lazy Evaluation

Transforms use lazy evaluation when possible:
```rust
df.lazy()
    .filter(condition)
    .select(columns)
    .collect()  // Executes optimized plan
```

### 3. Async I/O

All I/O operations are async, preventing blocking:
```rust
#[tokio::main]
async fn main() -> Result<()> {
    let pipeline = Pipeline::from_file(&config).await?;
    pipeline.execute(continue_on_error).await?;
    Ok(())
}
```

### 4. Memory Efficiency

- Streaming support for large files (future)
- In-place operations where possible
- Efficient serialization with `serde`

## Future Enhancements

### Completed ✅

1. **Dynamic Plugin System**: True runtime plugin loading
   - Workspace architecture with separate plugin crates
   - On-demand loading (plugins NOT in binary by default)
   - Panic isolation with `std::panic::catch_unwind`
   - Plugin API version checking
   - Capability verification
   - Platform-specific library loading (.dylib, .so, .dll)

2. **HTTP Plugin**: REST API integration
   - Source and sink implementation
   - Multiple HTTP methods (GET/POST/PUT/PATCH/DELETE)
   - Format support (JSON, JSONL, CSV, raw)
   - Custom headers and timeout configuration

3. **MongoDB Source**: Database integration with cursor-based pagination

4. **Workspace Dependencies**: Centralized dependency management
   - `[workspace.dependencies]` for version consistency
   - Shared dependency resolution across all crates
   - Independent plugin compilation

### Short Term

1. **Database Connectors**: PostgreSQL, MySQL implementations
2. **Advanced Transforms**:
   - `aggregate`: GROUP BY operations
   - `join`: Merge multiple data sources
   - `pivot`: Reshape data
3. **Authentication**: OAuth, API key support for HTTP plugin

### Medium Term

1. **FFI-Safe Plugin Interface**: Enhance cross-compiler compatibility
   - Explore WebAssembly as plugin format for sandboxing
   - Investigate simplified FFI types for core operations
   - Consider plugin protocol versioning

2. **Stream Processing**: Process data in chunks for memory efficiency
   - Implement streaming for large datasets
   - Add backpressure handling
   - Memory-bounded processing

3. **Monitoring**: Metrics, progress tracking, performance profiling
   - Pipeline execution metrics
   - Data throughput monitoring
   - Plugin performance tracking

### Long Term

1. **Distributed Execution**: Multi-node processing
2. **Web UI**: Visual pipeline builder and monitoring
3. **Scheduling**: Cron-like execution management
4. **Data Catalog**: Metadata management and lineage tracking

## Dependencies

### Workspace Dependencies

All common dependencies are managed in `[workspace.dependencies]` for consistency:

```toml
[workspace.dependencies]
# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Async runtime
tokio = { version = "1.47", features = ["full"] }
async-trait = "0.1"

# Error handling
anyhow = "1.0"
thiserror = "2.0"

# Logging
tracing = "0.1"

# Plugin API
conveyor-plugin-api = { path = "conveyor-plugin-api" }

# HTTP client (for HTTP plugin)
reqwest = { version = "0.12", features = ["json", "stream"] }

# Database (for MongoDB plugin)
mongodb = "3.3"
chrono = "0.4"
futures = "0.3.31"
```

### Main Binary Dependencies

```toml
[dependencies]
# CLI
clap = { version = "4.5", features = ["derive", "env"] }

# Workspace dependencies
serde = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
# ... etc

# Data processing (main binary only)
polars = { version = "0.44", features = ["lazy", "csv", "json", "parquet"] }
arrow = "54.3"

# Plugin system
libloading = "0.8"
conveyor-plugin-api = { workspace = true }
```

**Benefits of Workspace Dependencies**:
- Consistent versions across all crates
- Single location for version updates
- Prevents dependency conflicts
- Shared feature flags

## Code Organization

### Workspace Structure

```
conveyor/
├── Cargo.toml                     # Workspace root with shared dependencies
├── conveyor-plugin-api/           # Plugin API crate (lib)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                 # Plugin traits and version constants
├── plugins/                       # Plugin crates (cdylib)
│   ├── conveyor-plugin-http/
│   │   ├── Cargo.toml             # HTTP plugin manifest
│   │   └── src/
│   │       └── lib.rs             # HTTP source & sink (~300 lines)
│   └── conveyor-plugin-mongodb/
│       ├── Cargo.toml             # MongoDB plugin manifest
│       └── src/
│           └── lib.rs             # MongoDB source & sink (~400 lines)
└── src/                           # Main application
    ├── main.rs                    # CLI entry point
    ├── lib.rs                     # Library exports
    ├── plugin_loader.rs           # Dynamic plugin loader (150 lines)
    ├── cli/
    │   └── mod.rs                 # CLI helpers (list, generate)
    ├── core/
    │   ├── mod.rs                 # Core module exports
    │   ├── config.rs              # Configuration types (240 lines)
    │   ├── error.rs               # Error types (71 lines)
    │   ├── pipeline.rs            # Execution engine (180 lines)
    │   ├── registry.rs            # Module registry (120 lines)
    │   └── traits.rs              # Core traits (158 lines)
    ├── modules/
    │   ├── mod.rs                 # Module exports
    │   ├── sources/
    │   │   ├── mod.rs             # Built-in source registration
    │   │   ├── csv.rs             # CSV source (119 lines)
    │   │   ├── json.rs            # JSON source (129 lines)
    │   │   └── stdin.rs           # Stdin source (91 lines)
    │   ├── transforms/
    │   │   ├── mod.rs             # Transform registration
    │   │   ├── filter.rs          # Filter transform (130 lines)
    │   │   ├── map.rs             # Map transform (143 lines)
    │   │   └── validate.rs        # Validation (157 lines)
    │   └── sinks/
    │       ├── mod.rs             # Sink registration
    │       ├── csv.rs             # CSV sink (72 lines)
    │       ├── json.rs            # JSON sink (166 lines)
    │       └── stdout.rs          # Stdout sink (121 lines)
    └── utils/
        └── mod.rs                 # Utilities (placeholder)
```

**Total Lines of Code**:
- Main binary: ~2,000 lines
- Plugin API: ~100 lines
- HTTP plugin: ~300 lines
- MongoDB plugin: ~400 lines
- **Total**: ~2,800 lines (excluding tests)

**Key Organization Principles**:
- **Separation**: Plugins are completely separate from main binary
- **Built-in vs Plugin**: CSV/JSON/Stdin are built-in, HTTP/MongoDB are plugins
- **Workspace Benefits**: Shared dependencies, independent compilation
- **Plugin Isolation**: Each plugin is a self-contained crate

## Build Configuration

### Release Profile

Optimized for production:
```toml
[profile.release]
opt-level = 3          # Maximum optimization
lto = true             # Link-time optimization
codegen-units = 1      # Better optimization
strip = true           # Remove debug symbols
```

### Development Profile

Fast compilation for development:
```toml
[profile.dev]
opt-level = 0
debug = true
```

## Error Handling Philosophy

1. **Early Validation**: Validate configurations before execution
2. **Descriptive Errors**: Use `thiserror` for clear error messages
3. **Graceful Degradation**: Support continue-on-error mode
4. **Retry Logic**: Configurable retry with exponential backoff
5. **Dead Letter Queue**: Save failed records for inspection

## Logging Strategy

Using `tracing` for structured logging:

```rust
tracing::info!("Loading pipeline configuration from {:?}", config);
tracing::debug!("Registered {} sources", count);
tracing::warn!("Source '{}' failed: {}", name, error);
tracing::error!("Pipeline execution failed: {}", error);
```

Log levels:
- `TRACE`: Detailed execution flow
- `DEBUG`: Module operations
- `INFO`: Major milestones
- `WARN`: Recoverable errors
- `ERROR`: Fatal errors

## Development Workflow

### Local Testing

```bash
# Run tests
cargo test

# Run with specific log level
RUST_LOG=debug cargo run -- run -c pipeline.toml

# Check for issues
cargo clippy

# Format code
cargo fmt
```

### CI/CD Pipeline (Future)

```yaml
- lint: cargo clippy
- test: cargo test --all-features
- build: cargo build --release
- benchmark: cargo bench
- deploy: publish to crates.io
```

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

3. **Modular Design**: Trait-based design enables extensibility
   - Core traits: `DataSource`, `Transform`, `Sink`
   - Plugin system built on dynamic trait object loading
   - Registry pattern for module management

### Plugin System Insights

4. **Dynamic Plugin Loading**: Significant architectural benefits
   - Reduced binary size (plugins not compiled in)
   - Zero overhead for unused features
   - Independent plugin development and versioning
   - User chooses which plugins to load

5. **FFI Complexity**: Creating FFI-safe Rust interfaces is challenging
   - Not all Rust types cross FFI boundaries (trait objects, complex enums)
   - `abi_stable` and `safer-ffi` have significant limitations
   - Shared compiler version is acceptable for many use cases
   - WebAssembly may be better alternative for sandboxing

6. **Panic Isolation is Critical**: Plugins can crash without affecting host
   - `std::panic::catch_unwind` is essential for plugin systems
   - Prevents cascade failures
   - Better error messages for debugging

7. **Version Checking**: API versioning prevents incompatible plugins
   - Simple version constant: `PLUGIN_API_VERSION`
   - Reject incompatible plugins early
   - Clear error messages about version mismatches

### Workspace Management

8. **Workspace Dependencies**: Centralized version management
   - `[workspace.dependencies]` eliminates version conflicts
   - Single source of truth for dependency versions
   - Consistent feature flags across crates
   - Easier to update and maintain

9. **Crate Organization**: Separation of concerns
   - Plugin API as separate crate
   - Each plugin as independent crate with `cdylib`
   - Main binary doesn't depend on plugins
   - Plugins depend only on plugin API

### Development Practices

10. **Test Early**: Tests help validate API compatibility
    - Polars API changes caught by tests
    - Integration tests validate end-to-end flows
    - Unit tests for each module

11. **Performance Matters**: Polars' performance is a key differentiator
    - 10-100x faster than Python alternatives
    - Zero-copy operations with Arrow
    - Lazy evaluation for query optimization

12. **Documentation as Development Tool**: Writing docs clarifies design
    - README.md for users
    - CLAUDE.md for technical details
    - Helps identify inconsistencies and gaps

## Contributing Guidelines

### Code Style

- Follow Rust conventions (rustfmt)
- Use meaningful variable names
- Add doc comments for public APIs
- Include tests for new features

### Pull Request Process

1. Create feature branch
2. Add tests
3. Update documentation
4. Ensure CI passes
5. Request review

## Contact & Support

- **GitHub Issues**: Bug reports and feature requests
- **Discussions**: Design discussions and Q&A
- **Documentation**: https://docs.rs/conveyor (future)

---

**Built with Claude Code** - This document was created during an interactive development session with Claude, demonstrating collaborative AI-assisted software engineering.
