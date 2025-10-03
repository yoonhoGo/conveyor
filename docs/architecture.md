# Architecture

Technical overview of Conveyor's architecture and design decisions.

## System Overview

Conveyor is a high-performance ETL tool built with Rust, featuring a modular architecture and dual plugin system (FFI + WASM).

```
┌─────────────────────────────────────────────────────┐
│                   CLI (Clap)                        │
│                  Command Parser                     │
└─────────────────────┬───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────┐
│              Pipeline Executor                      │
│  ┌──────────────────────────────────────────────┐  │
│  │         DAG Builder & Executor               │  │
│  │  - Topological Sort                          │  │
│  │  - Level-based Parallelism                   │  │
│  │  - Cycle Detection                           │  │
│  └──────────────────────────────────────────────┘  │
└─────────────┬──────────────────┬────────────────────┘
              │                  │
    ┌─────────▼────────┐  ┌─────▼──────────┐
    │ Module Registry  │  │ Plugin Loader  │
    │ - Sources        │  │ - FFI Plugins  │
    │ - Transforms     │  │ - WASM Plugins │
    │ - Sinks          │  │ - Version Check│
    └─────────┬────────┘  └─────┬──────────┘
              │                  │
    ┌─────────▼──────────────────▼──────────┐
    │         Data Processing Layer         │
    │  ┌──────────────┐  ┌────────────────┐ │
    │  │ Polars       │  │ Arrow          │ │
    │  │ DataFrame    │  │ Columnar Format│ │
    │  └──────────────┘  └────────────────┘ │
    └───────────────────────────────────────┘
```

## Workspace Structure

```
conveyor/
├── Cargo.toml                     # Workspace root
├── conveyor-plugin-api/           # FFI Plugin API crate
├── conveyor-wasm-plugin-api/      # WASM Plugin API crate
├── plugins/                       # FFI Plugin crates
│   ├── conveyor-plugin-http/
│   └── conveyor-plugin-mongodb/
├── plugins-wasm/                  # WASM Plugin crates
│   └── conveyor-plugin-echo-wasm/
└── src/                           # Main application
    ├── cli/                       # Command-line interface
    ├── core/                      # Core pipeline engine
    ├── modules/                   # Built-in modules
    └── main.rs
```

## Core Components

### 1. Configuration System (`core/config.rs`)

Parses TOML configuration into strongly-typed structs:

```rust
pub struct DagPipelineConfig {
    pub pipeline: PipelineMetadata,
    pub global: GlobalConfig,
    pub stages: Vec<StageConfig>,
    pub error_handling: ErrorHandlingConfig,
}

pub struct StageConfig {
    pub id: String,
    pub stage_type: String,
    pub inputs: Vec<String>,
    pub config: HashMap<String, toml::Value>,
}
```

**Features:**
- DAG-based configuration
- Default values for optional fields
- Comprehensive validation
- Cycle detection
- Input validation

### 2. DAG Executor (`core/dag_executor.rs`)

Executes pipeline stages in topological order:

```rust
pub struct DagExecutor {
    graph: DiGraph<StageNode, ()>,
    node_map: HashMap<String, NodeIndex>,
    error_strategy: ErrorStrategy,
}
```

**Execution Flow:**
1. Build directed graph from stage dependencies
2. Perform topological sort to get execution order
3. Calculate execution levels for parallelism
4. Execute stages level-by-level
5. Pass data between stages via HashMap

**Level-Based Parallelism:**
```rust
// Stages in same level execute in parallel
for level in levels {
    let tasks: Vec<_> = level.iter()
        .map(|&node| tokio::spawn(execute_stage(node)))
        .collect();

    futures::future::join_all(tasks).await;
}
```

### 3. Module Registry (`core/registry.rs`)

Manages built-in and plugin modules:

```rust
pub struct ModuleRegistry {
    sources: HashMap<String, DataSourceRef>,
    transforms: HashMap<String, TransformRef>,
    sinks: HashMap<String, SinkRef>,
}
```

**Registration:**
- Built-in modules registered at startup
- Plugin modules registered on demand
- Type-safe registration with Arc wrappers

### 4. Plugin System

#### FFI Plugin Loader (`plugin_loader.rs`)

Loads dynamic libraries at runtime:

```rust
pub struct PluginLoader {
    plugin_dir: PathBuf,
    plugins: HashMap<String, PluginHandle>,
}

impl PluginLoader {
    pub fn load_plugin(&mut self, name: &str) -> Result<()> {
        // 1. Load dynamic library
        let library = unsafe { Library::new(lib_path)? };

        // 2. Get plugin declaration
        let decl: Symbol<&PluginDeclaration> =
            unsafe { library.get(b"_plugin_declaration")? };

        // 3. Verify API version
        if decl.api_version != PLUGIN_API_VERSION {
            return Err(anyhow!("Version mismatch"));
        }

        // 4. Register modules
        (decl.register)(&mut registry);

        Ok(())
    }
}
```

**Safety Features:**
- Panic isolation with `std::panic::catch_unwind`
- API version checking
- Platform-specific library loading
- Capability verification

#### WASM Plugin Loader (`wasm_plugin_loader.rs`)

Executes WebAssembly plugins in sandboxed environment:

```rust
pub struct WasmPluginLoader {
    engine: Engine,
    plugins: HashMap<String, WasmPluginHandle>,
}
```

**Features:**
- WebAssembly Component Model (WASI Preview 2)
- Complete memory isolation
- Capability-based security

### 5. Data Format (`core/traits.rs`)

Unified data representation:

```rust
pub enum DataFormat {
    DataFrame(DataFrame),      // Polars columnar format
    RecordBatch(RecordBatch),  // Vec<HashMap> row format
    Raw(Vec<u8>),              // Raw bytes
}
```

**Automatic Conversion:**
```rust
impl From<DataFrame> for DataFormat { ... }
impl From<RecordBatch> for DataFormat { ... }
impl TryInto<DataFrame> for DataFormat { ... }
```

### 6. Trait System

Core abstractions for extensibility:

```rust
#[async_trait]
pub trait DataSource: Send + Sync {
    async fn name(&self) -> &str;
    async fn read(&self, config: &HashMap<String, toml::Value>)
        -> Result<DataFormat>;
    async fn validate_config(&self, config: &HashMap<String, toml::Value>)
        -> Result<()>;
}

#[async_trait]
pub trait Transform: Send + Sync {
    async fn name(&self) -> &str;
    async fn apply(&self, data: DataFormat,
                    config: &Option<HashMap<String, toml::Value>>)
        -> Result<DataFormat>;
    async fn validate_config(&self, config: &Option<HashMap<String, toml::Value>>)
        -> Result<()>;
}

#[async_trait]
pub trait Sink: Send + Sync {
    async fn name(&self) -> &str;
    async fn write(&self, data: DataFormat,
                    config: &HashMap<String, toml::Value>)
        -> Result<()>;
    async fn validate_config(&self, config: &HashMap<String, toml::Value>>)
        -> Result<()>;
}
```

## Plugin Architecture

### FFI Plugin System (using `abi_stable`)

**Plugin API Crate:**
```rust
// conveyor-plugin-api/src/lib.rs
use abi_stable::{std_types::*, StableAbi};

#[repr(C)]
#[derive(StableAbi)]
pub enum FfiDataFormat {
    ArrowIpc(RVec<u8>),
    JsonRecords(RVec<u8>),
    Raw(RVec<u8>),
}

#[sabi_trait]
pub trait FfiDataSource: Send + Sync {
    fn read(&self, config: RHashMap<RString, RString>)
        -> RResult<FfiDataFormat, RBoxError>;
}
```

**Plugin Implementation:**
```rust
// plugins/conveyor-plugin-http/src/lib.rs
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("http"),  // Use rstr! macro for const
    version: rstr!("0.3.0"),
    description: rstr!("HTTP source and sink"),
    register,
};

#[no_mangle]
pub extern "C" fn register(registry: &mut dyn PluginRegistry) {
    registry.register_source(Box::new(HttpSource));
    registry.register_sink(Box::new(HttpSink));
}
```

### WASM Plugin System (Component Model)

**WIT Interface:**
```wit
// conveyor-wasm-plugin-api/wit/conveyor-plugin.wit
package conveyor:plugin;

interface types {
    record data-format {
        format-type: string,
        data: list<u8>,
    }
}

interface plugin {
    use types.{data-format};

    read: func(config: string) -> result<data-format, string>;
    write: func(data: data-format, config: string) -> result<_, string>;
}
```

## Key Design Decisions

### 1. Workspace Dependencies

All common dependencies in `[workspace.dependencies]`:

```toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.47", features = ["full"] }
polars = { version = "0.45", features = ["json", "csv"] }
```

**Benefits:**
- Version consistency across crates
- Easier dependency updates
- Prevents version conflicts

### 2. Plugin Isolation

Plugins are separate `cdylib` crates:

```toml
[lib]
crate-type = ["cdylib"]
```

**Benefits:**
- Independent compilation
- No binary bloat from unused plugins
- User chooses which plugins to load
- Independent versioning

### 3. Async Everywhere

All I/O operations are async:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let pipeline = DagPipeline::from_file(&config).await?;
    pipeline.execute().await?;
    Ok(())
}
```

**Benefits:**
- Efficient I/O with Tokio
- Parallel stage execution
- Non-blocking operations

### 4. Zero-Copy Operations

Polars uses Apache Arrow columnar format:

```rust
let df = df.lazy()
    .filter(condition)    // No copy
    .select(columns)      // No copy
    .collect()?;          // Execute
```

**Benefits:**
- Minimal memory allocation
- Efficient data processing
- Lazy evaluation optimizations

### 5. Type Safety

Strong typing throughout:

```rust
// Config is strongly typed
let config: DagPipelineConfig = toml::from_str(content)?;

// Stages are type-safe
let stage: StageRef = create_stage(&config)?;

// Data formats are enums
let data = DataFormat::DataFrame(df);
```

## Performance Optimizations

### 1. Polars DataFrame

- **Columnar format**: Efficient memory layout
- **Lazy evaluation**: Optimize query plans
- **SIMD operations**: Vectorized computations
- **Arrow backend**: Zero-copy data sharing

### 2. Tokio Runtime

- **Work-stealing scheduler**: Efficient task distribution
- **Async I/O**: Non-blocking operations
- **Parallel execution**: Multiple stages concurrently

### 3. Plugin System

- **Dynamic loading**: Only load needed plugins
- **No serialization overhead** (FFI): Direct memory access
- **Compile-time optimizations**: Release builds are optimized

## Error Handling

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConveyorError {
    #[error("Pipeline error: {0}")]
    PipelineError(String),

    #[error("Module not found: {0}")]
    ModuleNotFound(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}
```

### Error Strategies

```rust
pub enum ErrorStrategy {
    Stop,      // Halt on first error
    Continue,  // Skip failed stages
    Retry,     // Retry with backoff
}
```

### Panic Isolation

Plugins are isolated:

```rust
let result = std::panic::catch_unwind(|| {
    plugin_loader.load_plugin(name)
});

match result {
    Ok(Ok(())) => Ok(()),
    Ok(Err(e)) => Err(e),
    Err(_) => Err(anyhow!("Plugin panicked")),
}
```

## Testing Strategy

### Unit Tests

Located in each module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = DagPipelineConfig::from_str(toml)?;
        assert!(config.validate().is_ok());
    }
}
```

### Integration Tests

Located in `tests/`:

```rust
#[tokio::test]
async fn test_dag_pipeline() {
    let pipeline = DagPipeline::from_file("test.toml").await?;
    pipeline.validate()?;
    pipeline.execute().await?;
    // Verify output
}
```

## See Also

- [Development](development.md) - Building and testing
- [Plugin System](plugin-system.md) - Plugin architecture
- [DAG Pipelines](dag-pipelines.md) - Execution model
