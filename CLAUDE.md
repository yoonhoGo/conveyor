# Conveyor - Development Notes (Claude Code)

This document provides technical details about the Conveyor project's architecture, implementation decisions, and development process for AI agents and developers working on the codebase.

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
    ├── core/
    ├── modules/
    ├── plugin_loader.rs           # FFI plugin loader
    └── wasm_plugin_loader.rs      # WASM plugin loader
```

**Key Design Decisions**:

1. **Workspace Dependencies**: All common dependencies (serde, tokio, anyhow, etc.) are defined in `[workspace.dependencies]` for version consistency across all crates
2. **Plugin Isolation**: Plugins are separate `cdylib` crates compiled as dynamic libraries, not linked into the main binary
3. **Independent Compilation**: Each plugin can be built separately with `cargo build -p plugin-name`
4. **Shared API Crate**: `conveyor-plugin-api` provides common types and traits used by both host and plugins

### Core Components

#### 1. Configuration System (`core/config.rs`)

The configuration system uses `serde` for TOML deserialization with strong typing. Conveyor uses a DAG-based pipeline configuration:

```rust
pub struct DagPipelineConfig {
    pub pipeline: PipelineMetadata,
    pub global: GlobalConfig,
    pub stages: Vec<StageConfig>,
    pub error_handling: ErrorHandlingConfig,
}

pub struct StageConfig {
    pub id: String,              // Unique stage identifier
    pub function: String,        // Function name: "csv.read", "filter.apply", "json.write"
    pub inputs: Vec<String>,     // List of stage IDs this stage depends on
    pub config: HashMap<String, toml::Value>,
}
```

Key features:
- DAG-based execution with support for parallel stages and branching
- Default values for optional fields
- Comprehensive validation (cycle detection, unique IDs, valid inputs)
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

#### 2. Stage System (`core/stage.rs`, `core/traits.rs`)

The core abstraction uses a unified **Stage** trait that replaces the legacy DataSource/Transform/Sink system:

```rust
#[async_trait]
pub trait Stage: Send + Sync {
    fn name(&self) -> &str;

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat>;

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()>;

    fn produces_output(&self) -> bool {
        true  // Most stages produce output; sinks override to return false
    }
}
```

**Key Benefits**:
- **Unified API**: All modules (sources, transforms, sinks) implement the same trait
- **Function-based naming**: Stages are registered as functions like "csv.read", "filter.apply", "json.write"
- **Composability**: Easy to chain and combine stages in DAG pipelines
- **Input flexibility**: `inputs` HashMap supports multiple inputs for advanced use cases

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

#### 4. DAG Pipeline Executor (`core/pipeline.rs`, `core/dag_executor.rs`, `core/dag_builder.rs`)

The DAG pipeline executor orchestrates data flow using directed acyclic graphs:

```rust
pub struct DagPipeline {
    config: DagPipelineConfig,
    registry: Arc<ModuleRegistry>,
    executor: DagExecutor,
    plugin_loader: PluginLoader,
}
```

Features:
- **DAG-based execution**: Supports parallel execution and branching
- **Topological ordering**: Stages execute in dependency order
- **Level-based parallelism**: Stages with no dependencies between them run in parallel
- **Dynamic plugin loading** on initialization
- **Data passing between stages** via HashMap
- **Timeout handling** at pipeline level
- **Error recovery** based on strategy (stop/continue/retry)
- **Cycle detection** during validation

**Plugin Integration**: The pipeline loads plugins specified in `config.global.plugins` during initialization:
```rust
pub async fn new(config: DagPipelineConfig) -> Result<Self> {
    let registry = Arc::new(ModuleRegistry::with_defaults().await?);

    // Load plugins dynamically
    let mut plugin_loader = PluginLoader::new();
    if !config.global.plugins.is_empty() {
        plugin_loader.load_plugins(&config.global.plugins)?;
    }

    // Build DAG executor with cycle detection
    let builder = DagPipelineBuilder::new(registry.clone());
    let executor = builder.build(&config)?;

    Ok(Self { config, registry, executor, plugin_loader })
}
```

#### 5. Module Registry (`core/registry.rs`)

The registry manages available stages using a function-based API:

```rust
pub struct ModuleRegistry {
    stages: HashMap<String, StageRef>,
}
```

**Function Registration**:
- Built-in functions: "csv.read", "json.write", "filter.apply", etc.
- Plugin functions: "mongodb.find", "http.get", etc.
- Supports dynamic registration via plugin system

**Simplified API**:
- `register_stage(name, stage)` / `register_function(name, stage)`
- `get_stage(name)` / `get_function(name)`
- `list_stages()` / `list_functions()`

Previous complex API (26 methods across sources/transforms/sinks) reduced to 6 core methods.

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

### Stage Implementation

All built-in modules implement the `Stage` trait with function-based naming:

#### Source Stages

**CSV Reader** (`modules/sources/csv.rs`) - Function: `csv.read`
- Uses Polars' `CsvReader` with optimizations
- Supports custom delimiters and headers
- Schema inference capabilities
- Implements `Stage::execute()` with no inputs required

**JSON Reader** (`modules/sources/json.rs`) - Function: `json.read`
- Multiple format support (records, jsonl, dataframe)
- Handles large files efficiently
- Converts to appropriate DataFormat

**Stdin Reader** (`modules/sources/stdin.rs`) - Function: `stdin.read`
- Reads from standard input
- Format detection (json, jsonl, csv, raw)
- Enables pipeline chaining with Unix tools

#### Transform Stages

**Filter** (`modules/transforms/filter.rs`) - Function: `filter.apply`
- Supports comparison operators: `==`, `!=`, `>`, `>=`, `<`, `<=`
- String operations: `contains`, `in`
- Works on DataFrame columns efficiently

**Map** (`modules/transforms/map.rs`) - Function: `map.apply`
- Simple expression evaluation
- Arithmetic operations: `+`, `-`, `*`, `/`
- Column creation and modification

**Validate Schema** (`modules/transforms/validate.rs`) - Function: `validate.schema`
- Required fields checking
- Type validation
- Null constraint enforcement
- Unique value validation

**HTTP Fetch** (`modules/transforms/http_fetch.rs`) - Function: `http.fetch`
- Per-row or batch HTTP requests
- Template-based URLs with Handlebars
- Custom headers and timeout support

**Window** (`modules/transforms/window.rs`) - Function: `window.apply`
- Tumbling, sliding, and session windows
- Stream processing support

**Aggregate Stream** (`modules/transforms/aggregate_stream.rs`) - Function: `aggregate.stream`
- Real-time aggregation (count, sum, avg, min, max)
- Group-by support

#### Sink Stages

**CSV Writer** (`modules/sinks/csv.rs`) - Function: `csv.write`
- Writes Polars DataFrame to CSV
- Custom delimiters and headers
- Automatic directory creation
- `produces_output() = false`

**JSON Writer** (`modules/sinks/json.rs`) - Function: `json.write`
- Multiple output formats
- Pretty printing option
- Efficient serialization

**Stdout Writer** (`modules/sinks/stdout.rs`) - Function: `stdout.write`
- Table, JSON, JSONL, CSV output formats
- Row limiting for preview
- Colorized output (potential future enhancement)

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

**Successful Solution**: Full FFI implementation using `abi_stable` crate

1. **abi_stable crate 0.11**: Provides ABI-stable types for cross-compiler compatibility
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

### 4. Workspace Dependency Management

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
- FFI plugin system (12 tests)
- Total: 28+ unit tests

### Integration Tests

**Key Test Patterns**:
1. **Positive tests**: Valid configurations and operations
2. **Negative tests**: Invalid configs, missing fields
3. **Edge cases**: Empty data, large files, special characters
4. **Plugin lifecycle**: Load, execute, unload
5. **Error recovery**: Graceful handling of plugin failures

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

## Plugin Systems Comparison

Conveyor supports two plugin architectures:

### FFI Plugins (using `abi_stable`)

**Technical Architecture:**
- Dynamic library loading (.dylib, .so, .dll)
- FFI-safe traits with `#[sabi_trait]` macro
- Direct memory access between host and plugin
- `#[repr(C)]` for C-compatible memory layout
- `StableAbi` derive macro for FFI-safe types
- FFI-safe types: `RString`, `RVec`, `RResult`, `RBoxError`, `RHashMap`

**Performance Characteristics:**
- Near-zero overhead, direct function calls
- Native async/await support
- No serialization overhead
- Shared process memory (plugins not sandboxed)

**Constraints:**
- macOS-only support (.dylib)
- Same or compatible Rust compiler version
- Potential for version mismatches

### WASM Plugins (using WebAssembly Component Model)

**Technical Architecture:**
- WebAssembly Component Model (WASI Preview 2)
- WIT (WebAssembly Interface Types) definitions
- Wasmtime runtime for execution
- Complete memory isolation
- Capability-based security with ResourceTable

**Performance Characteristics:**
- 5-15% overhead vs FFI
- Serialization required for data exchange (Arrow IPC, JSON)
- Memory-safe by design
- Limited async support (WASI Preview 2 constraints)

**Constraints:**
- Ecosystem still maturing
- Wasmtime dependency (28.0+)
- `wasm32-wasip2` build target required

## Code Organization

**Key Organization Principles**:
- **Dual Plugin Systems**: FFI for performance, WASM for security/portability
- **Built-in vs Plugin**: CSV/JSON/Stdin are built-in, HTTP/MongoDB are FFI plugins
- **Workspace Benefits**: Shared dependencies, independent compilation
- **Plugin Isolation**: Each plugin is a self-contained crate
- **Cross-platform**: WASM plugins compile once, run everywhere

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
   - Registry pattern for module management

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
   - Key discovery: Use `RStr::from_str()` for const initialization was WRONG
   - **Correct approach**: Use `rstr!("literal")` macro in static declarations
   - Plugin declaration pattern:
     ```rust
     #[no_mangle]
     pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
         api_version: PLUGIN_API_VERSION,
         name: rstr!("plugin_name"),  // Critical: rstr! macro for static
         ...
     };
     ```

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
