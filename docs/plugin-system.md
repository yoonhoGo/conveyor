# Plugin System

Conveyor uses a **dynamic plugin system** that loads plugins only when needed, providing a flexible and safe way to extend functionality.

## How It Works

1. **On-Demand Loading**: Plugins specified in `[global].plugins` are loaded at runtime
2. **Version Checking**: API version compatibility is verified before loading
3. **Panic Isolation**: Plugin panics are caught and don't crash the host process
4. **Zero Overhead**: Unused plugins are never loaded into memory

## Available Plugins

| Plugin | Type | Description | Config in TOML |
|--------|------|-------------|----------------|
| `http` | Source & Sink | REST API integration | `plugins = ["http"]` |
| `mongodb` | Source & Sink | MongoDB database | `plugins = ["mongodb"]` |

## Using Plugins

### 1. Enable Plugin in Configuration

Add the plugin name to the `plugins` array in `[global]`:

```toml
[global]
log_level = "info"
plugins = ["http", "mongodb"]  # Load these plugins
```

### 2. Use Plugin Stages

Reference plugin stages with the `plugin.` prefix:

```toml
[[stages]]
id = "fetch_api"
type = "plugin.http"  # HTTP plugin
inputs = []

[stages.config]
url = "https://api.example.com/data"
method = "GET"
```

## HTTP Plugin

### Source (Fetch Data)

```toml
[global]
plugins = ["http"]

[[stages]]
id = "api_data"
type = "plugin.http"
inputs = []

[stages.config]
url = "https://jsonplaceholder.typicode.com/users"
method = "GET"
format = "json"  # json, jsonl, csv, raw
timeout = 30

[stages.config.headers]
Authorization = "Bearer YOUR_TOKEN"
Accept = "application/json"
```

### Sink (Send Data)

```toml
[[stages]]
id = "send_results"
type = "plugin.http"
inputs = ["processed_data"]

[stages.config]
url = "https://api.example.com/results"
method = "POST"
format = "json"

[stages.config.headers]
Content-Type = "application/json"
X-API-Key = "YOUR_KEY"
```

### Configuration Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `url` | ✅ Yes | - | Target URL |
| `method` | No | `GET` | HTTP method (GET, POST, PUT, PATCH, DELETE) |
| `format` | No | `json` | Data format (json, jsonl, csv, raw) |
| `timeout` | No | 30 | Request timeout in seconds |
| `headers` | No | - | Custom HTTP headers (table) |
| `body` | No | - | Request body (for POST/PUT/PATCH) |

## MongoDB Plugin

### Source (Read Data)

```toml
[global]
plugins = ["mongodb"]

[[stages]]
id = "user_events"
type = "plugin.mongodb"
inputs = []

[stages.config]
connection_string = "mongodb://localhost:27017"
database = "analytics"
collection = "events"
query = '{ "event_type": "purchase" }'

# Cursor-based pagination
cursor_field = "_id"
batch_size = 5000
limit = 10000  # Optional
```

### Sink (Write Data)

```toml
[[stages]]
id = "save_to_mongo"
type = "plugin.mongodb"
inputs = ["processed"]

[stages.config]
connection_string = "mongodb://localhost:27017"
database = "results"
collection = "processed_events"
batch_size = 1000  # Batch inserts
```

### Configuration Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `connection_string` | ✅ Yes | - | MongoDB connection URI |
| `database` | ✅ Yes | - | Database name |
| `collection` | ✅ Yes | - | Collection name |
| `query` | No | `{}` | Query filter (JSON string) |
| `cursor_field` | No | `_id` | Field for pagination |
| `cursor_value` | No | - | Starting cursor value |
| `batch_size` | No | 1000 | Documents per batch |
| `limit` | No | - | Total document limit |

## Plugin Safety

The plugin system includes several safety features:

### 1. Version Checking

Plugins are rejected if API version mismatches:

```rust
pub const PLUGIN_API_VERSION: &str = "0.3.0";

// During load:
if plugin.api_version != PLUGIN_API_VERSION {
    return Err(anyhow!("Plugin API version mismatch"));
}
```

### 2. Panic Handling

`std::panic::catch_unwind` isolates plugin failures:

```rust
let result = std::panic::catch_unwind(|| {
    plugin_loader.load_plugin(name)
});

match result {
    Ok(Ok(())) => Ok(()),
    Ok(Err(e)) => Err(e),
    Err(_) => Err(anyhow!("Plugin panicked during load")),
}
```

If a plugin panics, only that plugin fails—the host process continues.

### 3. Capability Verification

Plugins must provide at least one module type:

```rust
if sources.is_empty() && transforms.is_empty() && sinks.is_empty() {
    return Err(anyhow!("Plugin provides no modules"));
}
```

### 4. Platform-Specific Loading

Automatic library extension detection:

```rust
#[cfg(target_os = "macos")]
const LIB_EXT: &str = "dylib";

#[cfg(target_os = "linux")]
const LIB_EXT: &str = "so";

#[cfg(target_os = "windows")]
const LIB_EXT: &str = "dll";
```

## Creating Custom Plugins

### 1. Create Plugin Crate

```toml
# Cargo.toml
[package]
name = "conveyor-plugin-custom"
version = "0.1.0"

[dependencies]
conveyor-plugin-api = { path = "../../conveyor-plugin-api" }
async-trait = "0.1"
anyhow = "1.0"

[lib]
crate-type = ["cdylib"]  # Important!
```

### 2. Implement Plugin Traits

```rust
use conveyor_plugin_api::*;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct CustomSource;

#[async_trait]
impl FfiDataSource for CustomSource {
    async fn name(&self) -> &str {
        "custom"
    }

    async fn read(&self, config: &HashMap<String, toml::Value>)
        -> Result<FfiDataFormat> {
        // Implementation
        Ok(FfiDataFormat::Raw(vec![]))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>)
        -> Result<()> {
        // Validation
        Ok(())
    }
}
```

### 3. Export Plugin Declaration

```rust
use abi_stable::std_types::RStr;

#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("custom"),  // Use rstr! macro
    version: rstr!("0.1.0"),
    description: rstr!("Custom plugin"),
    register,
};

#[no_mangle]
pub extern "C" fn register(registry: &mut dyn PluginRegistry) {
    registry.register_source(Box::new(CustomSource));
}
```

### 4. Build Plugin

```bash
cargo build --release -p conveyor-plugin-custom

# Output: target/release/libconveyor_plugin_custom.dylib (macOS)
#         target/release/libconveyor_plugin_custom.so (Linux)
```

### 5. Use Plugin

```toml
[global]
plugins = ["custom"]

[[stages]]
id = "use_custom"
type = "plugin.custom"
inputs = []
```

## Plugin Directory Structure

```
plugins/
├── conveyor-plugin-http/
│   ├── src/
│   │   └── lib.rs           # HTTP source & sink implementation
│   └── Cargo.toml
├── conveyor-plugin-mongodb/
│   ├── src/
│   │   └── lib.rs           # MongoDB source & sink implementation
│   └── Cargo.toml
└── conveyor-plugin-custom/  # Your custom plugin
    ├── src/
    │   └── lib.rs
    └── Cargo.toml
```

## FFI-Safe Types

Use `abi_stable` types for FFI safety:

```rust
use abi_stable::{
    std_types::{RString, RVec, RResult, RHashMap},
    StableAbi,
};

#[repr(C)]
#[derive(StableAbi)]
pub enum FfiDataFormat {
    ArrowIpc(RVec<u8>),      // Polars DataFrame as Arrow IPC
    JsonRecords(RVec<u8>),   // JSON serialized records
    Raw(RVec<u8>),           // Raw bytes
}
```

**Key rules:**
- Use `RString` instead of `String`
- Use `RVec<T>` instead of `Vec<T>`
- Use `RHashMap` instead of `HashMap`
- Use `#[repr(C)]` for struct layout
- Derive `StableAbi` for all types

## Error Handling

Plugins return FFI-safe errors:

```rust
pub type FfiResult<T> = RResult<T, RBoxError>;

impl FfiDataSource for MySource {
    async fn read(&self, config: &HashMap<String, toml::Value>)
        -> FfiResult<FfiDataFormat> {

        let value = config.get("key")
            .ok_or_else(|| anyhow!("Missing key"))?;

        // ... implementation

        Ok(FfiDataFormat::Raw(data.into()))
    }
}
```

## Best Practices

1. **Version Consistency**: Match `PLUGIN_API_VERSION` with host
2. **Error Messages**: Provide clear, actionable error messages
3. **Configuration Validation**: Validate config in `validate_config()`
4. **Documentation**: Document all config options
5. **Testing**: Test plugin independently before integration
6. **Panic Safety**: Avoid panics; return errors instead

## Troubleshooting

### Plugin Not Found

```
Error: Plugin 'custom' not found in plugins directory
```

**Solution**: Ensure plugin library is in `target/release/` or configured plugin directory

### API Version Mismatch

```
Error: Plugin API version mismatch: plugin=0.2.0, host=0.3.0
```

**Solution**: Update plugin to match host API version

### Plugin Panicked

```
Error: Plugin 'custom' panicked during loading: ...
```

**Solution**: Check plugin code for panics, use `Result` for error handling

### Missing Symbol

```
Error: Symbol '_plugin_declaration' not found
```

**Solution**: Ensure `#[no_mangle]` and correct export:
```rust
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = ...;
```

## See Also

- [Architecture](architecture.md) - System design and plugin architecture
- [Development](development.md) - Building and testing plugins
- [Examples](../examples/plugin-template/) - Plugin template
