# Plugin Development Guide

Guide for creating custom plugins to extend Conveyor with new sources, transforms, and sinks.

## Overview

Conveyor supports two plugin types:

- **FFI Plugins**: High-performance, Rust-to-Rust dynamic libraries
- **WASM Plugins**: Cross-platform, sandboxed WebAssembly modules

## Plugin Architecture

### Dynamic Loading

Plugins are loaded on-demand based on pipeline configuration:

```toml
[global]
plugins = ["http", "mongodb", "my_custom_plugin"]
```

### Safety Features

1. **Version Checking**: API version compatibility verification
2. **Panic Isolation**: Plugin crashes don't affect host process
3. **Lazy Loading**: Only specified plugins are loaded
4. **Platform Detection**: Automatic library extension (.dylib, .so, .dll)

## FFI Plugin Development

FFI plugins offer near-zero overhead for performance-critical operations.

### 1. Create Plugin Crate

```bash
cd plugins
cargo new --lib conveyor-plugin-custom
```

**Cargo.toml:**

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
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

[lib]
crate-type = ["cdylib"]  # Important: Dynamic library
```

### 2. Implement Plugin Traits

```rust
// src/lib.rs
use conveyor_plugin_api::*;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct CustomSource;

#[async_trait]
impl FfiDataSource for CustomSource {
    async fn name(&self) -> &str {
        "custom"
    }

    async fn read(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<FfiDataFormat> {
        // Extract configuration
        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' configuration"))?;

        // Fetch data
        let data = fetch_data(url).await?;

        // Convert to FFI-safe format
        Ok(FfiDataFormat::JsonRecords(data.into()))
    }

    async fn validate_config(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<()> {
        // Validate required fields
        if !config.contains_key("url") {
            return Err(anyhow::anyhow!("Missing required 'url' field").into());
        }
        Ok(())
    }
}

async fn fetch_data(url: &str) -> anyhow::Result<Vec<u8>> {
    // Implementation
    Ok(vec![])
}
```

### 3. Export Plugin Declaration

**Critical**: Use `rstr!` macro for static string initialization:

```rust
use abi_stable::std_types::{RStr, RString};

#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("custom"),          // Use rstr! macro
    version: rstr!("0.1.0"),
    description: rstr!("Custom data source plugin"),
    register,
};

#[no_mangle]
pub extern "C" fn register(registry: &mut dyn PluginRegistry) {
    registry.register_source(Box::new(CustomSource));
}
```

**Why `rstr!`?**: `RStr::from()` and `RStr::from_str()` fail in static context. The `rstr!` macro is essential for const initialization.

### 4. Build Plugin

```bash
cargo build --release -p conveyor-plugin-custom
```

Output: `target/release/libconveyor_plugin_custom.{dylib,so,dll}`

### 5. Use Plugin

```toml
[global]
plugins = ["custom"]

[[stages]]
id = "load_custom"
function = "custom"
inputs = []
[stages.config]
url = "https://example.com/data"
```

## FFI-Safe Types

Use `abi_stable` types for cross-compiler compatibility:

### Type Mappings

| Rust Type | FFI-Safe Type |
|-----------|---------------|
| `String` | `RString` |
| `Vec<T>` | `RVec<T>` |
| `HashMap<K, V>` | `RHashMap<K, V>` |
| `Result<T, E>` | `RResult<T, E>` |
| `Box<dyn Error>` | `RBoxError` |

### Data Format

```rust
#[repr(C)]
#[derive(StableAbi)]
pub enum FfiDataFormat {
    ArrowIpc(RVec<u8>),      // Polars DataFrame as Arrow IPC
    JsonRecords(RVec<u8>),   // JSON serialized records
    Raw(RVec<u8>),           // Raw bytes
}
```

### Traits

```rust
use abi_stable::sabi_trait;

#[sabi_trait]
pub trait FfiDataSource: Send + Sync {
    async fn name(&self) -> &str;
    async fn read(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<FfiDataFormat>;
    async fn validate_config(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<()>;
}
```

## WASM Plugin Development

WASM plugins offer cross-platform compatibility and sandboxed execution.

### 1. Create WASM Plugin Crate

```bash
cd plugins-wasm
cargo new --lib conveyor-plugin-custom-wasm
```

**Cargo.toml:**

```toml
[package]
name = "conveyor-plugin-custom-wasm"
version = "0.1.0"
edition = "2021"

[dependencies]
conveyor-wasm-plugin-api = { path = "../../conveyor-wasm-plugin-api" }
wit-bindgen = "0.16"

[lib]
crate-type = ["cdylib"]
```

### 2. Implement WASM Interface

```rust
// src/lib.rs
wit_bindgen::generate!({
    world: "conveyor-plugin",
    exports: {
        "conveyor:plugin/source": CustomSource,
    },
});

struct CustomSource;

impl Guest for CustomSource {
    fn transform(input: Vec<u8>) -> Vec<u8> {
        // Parse JSON records
        let records: Vec<HashMap<String, serde_json::Value>> =
            serde_json::from_slice(&input).unwrap();

        // Transform data
        let transformed = records.into_iter()
            .map(|mut record| {
                // Custom transformation logic
                record.insert("processed".to_string(), true.into());
                record
            })
            .collect::<Vec<_>>();

        // Serialize back to JSON
        serde_json::to_vec(&transformed).unwrap()
    }
}
```

### 3. Build WASM Plugin

Install WASM target:

```bash
rustup target add wasm32-wasip2
```

Build:

```bash
cargo build --release -p conveyor-plugin-custom-wasm --target wasm32-wasip2
```

Output: `target/wasm32-wasip2/release/conveyor_plugin_custom_wasm.wasm`

### 4. Use WASM Plugin

```toml
[global]
plugins = ["custom-wasm"]

[[stages]]
id = "transform_custom"
function = "custom-wasm"
inputs = ["data"]
```

## Plugin Capabilities

### Data Sources

Implement `FfiDataSource` trait:

```rust
#[async_trait]
impl FfiDataSource for MySource {
    async fn name(&self) -> &str {
        "my_source"
    }

    async fn read(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<FfiDataFormat> {
        // Fetch data from external source
        // Convert to FfiDataFormat
        Ok(FfiDataFormat::JsonRecords(data.into()))
    }

    async fn validate_config(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<()> {
        // Validate required configuration fields
        Ok(())
    }
}
```

### Transforms

Implement `FfiTransform` trait:

```rust
#[async_trait]
impl FfiTransform for MyTransform {
    async fn name(&self) -> &str {
        "my_transform"
    }

    async fn transform(
        &self,
        input: FfiDataFormat,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<FfiDataFormat> {
        // Transform data
        // Return modified FfiDataFormat
        Ok(input)
    }

    async fn validate_config(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<()> {
        Ok(())
    }
}
```

### Sinks

Implement `FfiSink` trait:

```rust
#[async_trait]
impl FfiSink for MySink {
    async fn name(&self) -> &str {
        "my_sink"
    }

    async fn write(
        &self,
        data: FfiDataFormat,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<()> {
        // Write data to external destination
        Ok(())
    }

    async fn validate_config(
        &self,
        config: &HashMap<String, toml::Value>,
    ) -> FfiResult<()> {
        Ok(())
    }
}
```

## Error Handling

### Return FFI-Safe Errors

```rust
use anyhow::anyhow;

async fn read(&self, config: &HashMap<String, toml::Value>)
    -> FfiResult<FfiDataFormat> {

    // Validation error
    let url = config.get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required 'url' field"))?;

    // Operation error
    let data = fetch_data(url)
        .await
        .map_err(|e| anyhow!("Failed to fetch data: {}", e))?;

    Ok(FfiDataFormat::JsonRecords(data.into()))
}
```

### Error Message Best Practices

1. **Be specific**: `"Missing required 'url' field"` not `"Invalid config"`
2. **Include context**: `"Failed to connect to {url}: {error}"`
3. **Suggest solutions**: `"Port must be between 1 and 65535"`

## Testing Plugins

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read() {
        let source = CustomSource;
        let mut config = HashMap::new();
        config.insert("url".to_string(), toml::Value::String("https://example.com".to_string()));

        let result = source.read(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_missing_url() {
        let source = CustomSource;
        let config = HashMap::new();

        let result = source.validate_config(&config).await;
        assert!(result.is_err());
    }
}
```

### Integration Tests

Create test pipeline:

```toml
# test_pipeline.toml
[global]
plugins = ["custom"]

[[stages]]
id = "test_source"
function = "custom"
inputs = []
[stages.config]
url = "https://jsonplaceholder.typicode.com/users"

[[stages]]
id = "output"
function = "stdout.write"
inputs = ["test_source"]
```

Test:

```bash
cargo build --release -p conveyor-plugin-custom
conveyor run test_pipeline.toml
```

## Best Practices

### 1. Configuration Validation

Always validate configuration early:

```rust
async fn validate_config(&self, config: &HashMap<String, toml::Value>)
    -> FfiResult<()> {

    // Check required fields
    if !config.contains_key("url") {
        return Err(anyhow!("Missing required 'url' field").into());
    }

    // Validate types
    let timeout = config.get("timeout")
        .and_then(|v| v.as_integer())
        .unwrap_or(30);

    if timeout < 1 || timeout > 300 {
        return Err(anyhow!("timeout must be between 1 and 300 seconds").into());
    }

    Ok(())
}
```

### 2. Use Workspace Dependencies

Add to workspace `Cargo.toml`:

```toml
[workspace.dependencies]
conveyor-plugin-api = { path = "conveyor-plugin-api" }

[dependencies]
conveyor-plugin-api = { workspace = true }
```

### 3. Document Configuration Options

Add comments to explain configuration:

```rust
// Configuration:
// - url (required): API endpoint URL
// - timeout (optional): Request timeout in seconds (default: 30)
// - headers (optional): Custom HTTP headers as key-value pairs
```

### 4. Handle Panics

Use `Result` instead of `panic!`:

```rust
// Good
let value = config.get("key")
    .ok_or_else(|| anyhow!("Missing 'key'"))?;

// Bad
let value = config.get("key").unwrap();  // Can panic!
```

### 5. Resource Cleanup

Implement proper cleanup:

```rust
impl Drop for MySource {
    fn drop(&mut self) {
        // Close connections, release resources
        self.connection.close();
    }
}
```

## FFI vs WASM Comparison

### Use FFI When:

- Performance is critical (near-zero overhead)
- Need direct memory access
- Using async/await heavily
- Same platform/compiler version is acceptable

### Use WASM When:

- Cross-platform compatibility required
- Sandboxed execution needed
- Untrusted code execution
- Independent compiler versions
- 5-15% overhead is acceptable

## Troubleshooting

### Plugin Not Found

**Error:** `Plugin 'custom' not found`

**Check:**
1. Plugin library is in `target/release/`
2. Library name matches: `libconveyor_plugin_custom.{dylib,so,dll}`
3. Plugin is listed in `[global].plugins`

### API Version Mismatch

**Error:** `Plugin API version mismatch`

**Solution:** Update plugin to match host API version:

```rust
pub const PLUGIN_API_VERSION: &str = "0.3.0";
```

### Symbol Not Found

**Error:** `Symbol '_plugin_declaration' not found`

**Check:**
1. `#[no_mangle]` attribute is present
2. Symbol is `pub static`
3. Using `rstr!` macro for strings
4. `crate-type = ["cdylib"]` in Cargo.toml

### Panic in Plugin

**Error:** `Plugin panicked during load`

**Debug:**
1. Add debug logging
2. Check `validate_config` implementation
3. Verify all required dependencies are available
4. Test plugin in isolation

## Plugin Distribution

### 1. Version Plugin

```toml
[package]
version = "1.0.0"
```

### 2. Build for Release

```bash
cargo build --release -p conveyor-plugin-custom
```

### 3. Package Plugin

```bash
mkdir -p dist
cp target/release/libconveyor_plugin_custom.{dylib,so,dll} dist/
```

### 4. Document Usage

Create `README.md`:

```markdown
# Custom Plugin

## Installation

Copy `libconveyor_plugin_custom.{dylib,so,dll}` to Conveyor's plugin directory.

## Configuration

\`\`\`toml
[global]
plugins = ["custom"]

[[stages]]
function = "custom"
[stages.config]
url = "https://example.com"
\`\`\`

## Options

- `url` (required): API endpoint URL
- `timeout` (optional): Request timeout (default: 30s)
```

## See Also

- **[Development Guide](development.md)** - Building and testing
- **[Metadata System](metadata-system.md)** - Self-documenting plugins
- **[HTTP Plugin](plugins/http.md)** - Example FFI plugin
- **[MongoDB Plugin](plugins/mongodb.md)** - Example FFI plugin
- **[CLAUDE.md](../CLAUDE.md)** - Architecture and implementation details
