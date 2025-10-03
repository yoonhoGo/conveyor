# Development Guide

Guide for developing and contributing to Conveyor.

## Prerequisites

- **Rust**: 1.70 or higher
- **Cargo**: Latest version
- **Git**: For version control

### Platform Requirements

**macOS only:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Xcode Command Line Tools
xcode-select --install
```

## Getting Started

### 1. Clone Repository

```bash
git clone https://github.com/yoonhoGo/conveyor.git
cd conveyor
```

### 2. Build Project

```bash
# Build all crates
cargo build --all

# Build in release mode
cargo build --all --release

# Build specific crate
cargo build -p conveyor
cargo build -p conveyor-plugin-http
```

### 3. Run Tests

```bash
# Run all tests
cargo test --all

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_dag_executor
```

### 4. Run Conveyor

```bash
# Development build
cargo run -- run -c examples/simple_pipeline.toml

# Release build
cargo run --release -- run -c examples/simple_pipeline.toml

# Or use built binary
./target/debug/conveyor run -c examples/simple_pipeline.toml
```

## Project Structure

```
conveyor/
├── src/
│   ├── cli/              # Command-line interface
│   │   ├── mod.rs
│   │   ├── add_stage.rs
│   │   ├── edit.rs
│   │   └── scaffold.rs
│   ├── core/             # Core pipeline engine
│   │   ├── config.rs     # Configuration parsing
│   │   ├── dag_builder.rs
│   │   ├── dag_executor.rs
│   │   ├── error.rs
│   │   ├── pipeline.rs   # Pipeline executor
│   │   ├── registry.rs   # Module registry
│   │   ├── stage.rs      # Stage adapters
│   │   ├── strategy.rs   # Error strategies
│   │   └── traits.rs     # Core traits
│   ├── modules/          # Built-in modules
│   │   ├── sources/
│   │   ├── transforms/
│   │   ├── sinks/
│   │   └── stages/
│   ├── plugin_loader.rs  # FFI plugin loader
│   ├── wasm_plugin_loader.rs
│   ├── utils/
│   ├── lib.rs
│   └── main.rs
├── conveyor-plugin-api/  # FFI Plugin API
├── plugins/              # FFI Plugins
└── tests/                # Integration tests
```

## Development Workflow

### 1. Create Feature Branch

```bash
git checkout -b feature/my-feature
```

### 2. Make Changes

```bash
# Edit code
vim src/modules/transforms/my_transform.rs

# Format code
cargo fmt

# Check for issues
cargo clippy --all-targets --all-features
```

### 3. Test Changes

```bash
# Run tests
cargo test

# Run specific tests
cargo test test_my_feature

# Check test coverage
cargo tarpaulin
```

### 4. Commit Changes

```bash
git add .
git commit -m "feat: add my feature"
```

### 5. Push and Create PR

```bash
git push origin feature/my-feature
# Create pull request on GitHub
```

## Building Plugins

### Create New FFI Plugin

**1. Create Plugin Crate:**

```bash
mkdir -p plugins/conveyor-plugin-custom
cd plugins/conveyor-plugin-custom
```

**2. Add Cargo.toml:**

```toml
[package]
name = "conveyor-plugin-custom"
version = "0.1.0"
edition = "2021"

[dependencies]
conveyor-plugin-api = { path = "../../conveyor-plugin-api" }
async-trait = "0.1"
anyhow = "1.0"
tokio = { version = "1.47", features = ["full"] }

[lib]
crate-type = ["cdylib"]
```

**3. Implement Plugin:**

```rust
// src/lib.rs
use conveyor_plugin_api::*;
use async_trait::async_trait;

pub struct CustomSource;

#[async_trait]
impl FfiDataSource for CustomSource {
    async fn name(&self) -> &str {
        "custom"
    }

    async fn read(&self, config: &HashMap<String, toml::Value>)
        -> Result<FfiDataFormat> {
        // Implementation
        Ok(FfiDataFormat::Raw(vec![].into()))
    }

    async fn validate_config(&self, _config: &HashMap<String, toml::Value>)
        -> Result<()> {
        Ok(())
    }
}

#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("custom"),
    version: rstr!("0.1.0"),
    description: rstr!("Custom plugin"),
    register,
};

#[no_mangle]
pub extern "C" fn register(registry: &mut dyn PluginRegistry) {
    registry.register_source(Box::new(CustomSource));
}
```

**4. Build Plugin:**

```bash
cargo build -p conveyor-plugin-custom --release
```

**5. Use Plugin:**

```toml
[global]
plugins = ["custom"]

[[stages]]
id = "use_custom"
type = "plugin.custom"
inputs = []
```

### Create WASM Plugin

**1. Install wasm32-wasip2 target:**

```bash
rustup target add wasm32-wasip2
```

**2. Create Plugin Crate:**

```toml
[package]
name = "conveyor-plugin-custom-wasm"
version = "0.1.0"

[dependencies]
conveyor-wasm-plugin-api = { path = "../../conveyor-wasm-plugin-api" }

[lib]
crate-type = ["cdylib"]
```

**3. Build WASM:**

```bash
cargo build -p conveyor-plugin-custom-wasm --target wasm32-wasip2 --release
```

## Code Standards

### Formatting

Use `rustfmt`:

```bash
cargo fmt --all
```

**Config:** `.rustfmt.toml`

```toml
max_width = 100
tab_spaces = 4
edition = "2021"
```

### Linting

Use `clippy`:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Documentation

Document all public APIs:

```rust
/// Executes the DAG pipeline
///
/// # Errors
///
/// Returns an error if:
/// - Cycle detected in DAG
/// - Stage execution fails
/// - Timeout exceeded
pub async fn execute(&self) -> Result<()> {
    // Implementation
}
```

Generate docs:

```bash
cargo doc --open
```

## Testing

### Unit Tests

Place tests in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        assert_eq!(2 + 2, 4);
    }

    #[tokio::test]
    async fn test_async_feature() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Integration Tests

Place in `tests/` directory:

```rust
// tests/my_integration_test.rs
use conveyor::*;

#[tokio::test]
async fn test_complete_pipeline() {
    let pipeline = DagPipeline::from_file("test.toml").await?;
    pipeline.validate()?;
    pipeline.execute().await?;
    // Assertions
}
```

### Test Coverage

Use `tarpaulin`:

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --all --out Html
open tarpaulin-report.html
```

## Benchmarking

Use `criterion`:

```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "pipeline_benchmark"
harness = false
```

```rust
// benches/pipeline_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_pipeline(c: &mut Criterion) {
    c.bench_function("execute pipeline", |b| {
        b.iter(|| {
            // Benchmark code
            black_box(pipeline.execute())
        });
    });
}

criterion_group!(benches, benchmark_pipeline);
criterion_main!(benches);
```

Run benchmarks:

```bash
cargo bench
```

## Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug cargo run -- run -c pipeline.toml
```

### Use Debugger

**VS Code** (`launch.json`):

```json
{
    "type": "lldb",
    "request": "launch",
    "name": "Debug Conveyor",
    "cargo": {
        "args": ["build", "--bin=conveyor"]
    },
    "args": ["run", "-c", "examples/simple_pipeline.toml"],
    "cwd": "${workspaceFolder}"
}
```

### Print Debugging

```rust
dbg!(&pipeline_config);
eprintln!("Debug: {:?}", value);
```

## Contributing

### Commit Message Format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new transform module
fix: resolve memory leak in plugin loader
docs: update README with examples
test: add integration tests for DAG executor
refactor: simplify error handling
perf: optimize DataFrame operations
```

### Pull Request Process

1. **Fork** repository
2. **Create** feature branch
3. **Make** changes with tests
4. **Run** `cargo fmt` and `cargo clippy`
5. **Ensure** all tests pass
6. **Submit** pull request with description

### Code Review

PRs require:
- All tests passing
- No clippy warnings
- Documentation for new features
- Test coverage for new code

## Release Process

### 1. Version Bump

Update version in `Cargo.toml`:

```toml
[package]
version = "0.4.0"
```

### 2. Update Changelog

Add to `CHANGELOG.md`:

```markdown
## [0.4.0] - 2024-01-01

### Added
- New feature X

### Fixed
- Bug in Y
```

### 3. Create Git Tag

```bash
git tag -a v0.4.0 -m "Release v0.4.0"
git push origin v0.4.0
```

### 4. Build Release

```bash
cargo build --release --all
```

### 5. Publish

```bash
# Publish to crates.io
cargo publish -p conveyor-plugin-api
cargo publish -p conveyor
```

## Troubleshooting

### Build Errors

**OpenSSL missing (macOS):**
```bash
brew install openssl
```

### Test Failures

**Timeout:**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_with_timeout() {
    tokio::time::timeout(
        Duration::from_secs(30),
        pipeline.execute()
    ).await??;
}
```

### Plugin Issues

**Symbol not found:**
- Ensure `#[no_mangle]` on exports
- Check API version compatibility
- Verify `cdylib` crate type

## See Also

- [Architecture](architecture.md) - System design
- [Plugin System](plugin-system.md) - Plugin development
- [Contributing Guidelines](../CONTRIBUTING.md)
