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
}
```

Features:
- Sequential execution of sources → transforms → sinks
- Data passing between stages
- Timeout handling
- Error recovery based on strategy (stop/continue/retry)

#### 5. Module Registry (`core/registry.rs`)

The registry manages available modules:

```rust
pub struct ModuleRegistry {
    sources: HashMap<String, DataSourceRef>,
    transforms: HashMap<String, TransformRef>,
    sinks: HashMap<String, SinkRef>,
}
```

Supports dynamic module registration for future plugin system.

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

## Testing Strategy

### Unit Tests

Located in each module using `#[cfg(test)]`:
- Config validation (5 tests)
- Registry operations (3 tests)
- Source modules (4 tests)
- Total: 12 unit tests

### Integration Tests

Located in `tests/integration_test.rs`:
- End-to-end pipeline scenarios
- File I/O with temporary directories
- Config parsing verification
- Total: 3 integration tests

### Test Coverage

```
test result: ok. 15 passed; 0 failed; 0 ignored
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

### Short Term

1. **Database Connectors**: Complete MongoDB, PostgreSQL, MySQL implementations
2. **HTTP Sources**: REST API data fetching with authentication
3. **Advanced Transforms**:
   - `aggregate`: GROUP BY operations
   - `join`: Merge multiple data sources
   - `pivot`: Reshape data

### Medium Term

1. **Plugin System**: Dynamic loading with `libloading`
   ```rust
   type PluginCreate = unsafe fn() -> *mut dyn Plugin;
   ```

2. **Stream Processing**: Process data in chunks for memory efficiency
3. **Monitoring**: Metrics, progress tracking, performance profiling

### Long Term

1. **Distributed Execution**: Multi-node processing
2. **Web UI**: Visual pipeline builder and monitoring
3. **Scheduling**: Cron-like execution management
4. **Data Catalog**: Metadata management and lineage tracking

## Dependencies

Key dependencies and their purposes:

```toml
[dependencies]
clap = "4.5"           # CLI argument parsing
serde = "1.0"          # Serialization framework
toml = "0.8"           # TOML parsing
tokio = "1.40"         # Async runtime
polars = "0.44"        # DataFrame processing
arrow = "54.0"         # Columnar memory format
reqwest = "0.12"       # HTTP client
mongodb = "3.1"        # MongoDB driver
sqlx = "0.8"           # SQL database toolkit
thiserror = "2.0"      # Error derive macros
anyhow = "1.0"         # Flexible error handling
tracing = "0.1"        # Structured logging
```

## Code Organization

```
src/
├── main.rs           # CLI entry point
├── lib.rs            # Library exports
├── cli/
│   └── mod.rs        # CLI helpers (list, generate)
├── core/
│   ├── mod.rs        # Core module exports
│   ├── config.rs     # Configuration types (227 lines)
│   ├── error.rs      # Error types (71 lines)
│   ├── pipeline.rs   # Execution engine (165 lines)
│   ├── registry.rs   # Module registry (129 lines)
│   └── traits.rs     # Core traits (155 lines)
├── modules/
│   ├── mod.rs        # Module exports
│   ├── sources/
│   │   ├── mod.rs    # Source registration
│   │   ├── csv.rs    # CSV source (119 lines)
│   │   ├── json.rs   # JSON source (129 lines)
│   │   └── stdin.rs  # Stdin source (91 lines)
│   ├── transforms/
│   │   ├── mod.rs    # Transform registration
│   │   ├── filter.rs # Filter transform (130 lines)
│   │   ├── map.rs    # Map transform (143 lines)
│   │   └── validate.rs # Validation (157 lines)
│   └── sinks/
│       ├── mod.rs    # Sink registration
│       ├── csv.rs    # CSV sink (72 lines)
│       ├── json.rs   # JSON sink (166 lines)
│       └── stdout.rs # Stdout sink (121 lines)
└── utils/
    └── mod.rs        # Utilities (placeholder)
```

Total: ~1,900 lines of code (excluding tests)

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

1. **Type Safety First**: Strong typing catches errors at compile time
2. **Async Everywhere**: Consistent async makes composition easier
3. **Modular Design**: Trait-based design enables extensibility
4. **Test Early**: Tests help validate API compatibility
5. **Performance Matters**: Polars' performance is a key differentiator

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
