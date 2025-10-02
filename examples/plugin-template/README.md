# Conveyor Plugin Template

This template provides a starting point for creating your own Conveyor plugins.

## Quick Start

1. **Copy this template:**
   ```bash
   cp -r examples/plugin-template plugins/conveyor-plugin-yourname
   cd plugins/conveyor-plugin-yourname
   ```

2. **Update `Cargo.toml`:**
   - Replace `YOURNAME` with your plugin name
   - Update author and description
   - Add any dependencies you need

3. **Update `src/lib.rs`:**
   - Replace `YourName` throughout with your plugin name
   - Implement your data source, transform, or sink logic
   - Update the plugin declaration at the bottom

4. **Build your plugin:**
   ```bash
   cargo build -p conveyor-plugin-yourname
   ```

5. **Add to workspace:**
   Edit the root `Cargo.toml` and add your plugin to the workspace members:
   ```toml
   members = [
       "conveyor-plugin-api",
       "plugins/conveyor-plugin-yourname",  # Add this line
   ]
   ```

## Plugin Structure

A Conveyor plugin can provide:

### Data Source
Reads data from an external source (API, database, file, etc.)

```rust
impl FfiDataSource for YourSource {
    fn name(&self) -> RStr<'_> { "your_source".into() }
    fn read(&self, config: RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
        // Your implementation
    }
    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Validate configuration
    }
}
```

### Transform
Transforms data in the pipeline

```rust
impl FfiTransform for YourTransform {
    fn name(&self) -> RStr<'_> { "your_transform".into() }
    fn apply(&self, data: FfiDataFormat, config: RHashMap<RString, RString>)
        -> RResult<FfiDataFormat, RBoxError> {
        // Your implementation
    }
    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Validate configuration
    }
}
```

### Sink
Writes data to an external destination

```rust
impl FfiSink for YourSink {
    fn name(&self) -> RStr<'_> { "your_sink".into() }
    fn write(&self, data: FfiDataFormat, config: RHashMap<RString, RString>)
        -> RResult<(), RBoxError> {
        // Your implementation
    }
    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Validate configuration
    }
}
```

## Data Formats

Conveyor supports three data formats:

1. **Arrow IPC** - Polars DataFrame serialized as Arrow IPC
   ```rust
   FfiDataFormat::from_arrow_ipc(bytes)
   ```

2. **JSON Records** - Vec<HashMap<String, Value>> as JSON
   ```rust
   FfiDataFormat::from_json_records(&records)?
   ```

3. **Raw Bytes** - Unstructured binary data
   ```rust
   FfiDataFormat::from_raw(bytes)
   ```

## Configuration

Your plugin receives configuration as `RHashMap<RString, RString>`:

```rust
let value = config.get(&RString::from("key"))
    .ok_or_else(|| RBoxError::from_fmt(&format_args!("Missing 'key' config")))?;
```

## Plugin Declaration

The plugin declaration is the entry point:

```rust
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: conveyor_plugin_api::PLUGIN_API_VERSION,
    name: rstr!("yourname"),
    version: rstr!("0.1.0"),
    description: rstr!("Your plugin description"),
    register,
};
```

**Important:**
- Must be named `_plugin_declaration`
- Must be a `static` variable
- Must use `#[no_mangle]`
- Use `rstr!()` macro for string literals in static context

## Testing

Run tests with:
```bash
cargo test -p conveyor-plugin-yourname
```

Example test:
```rust
#[test]
fn test_source() {
    let source = YourSource;
    let config = RHashMap::new();
    let result = source.read(config);
    assert!(result.is_ok());
}
```

## Using Your Plugin

1. Build your plugin:
   ```bash
   cargo build --release -p conveyor-plugin-yourname
   ```

2. The compiled plugin will be at:
   - macOS: `target/release/libconveyor_plugin_yourname.dylib`
   - Linux: `target/release/libconveyor_plugin_yourname.so`
   - Windows: `target/release/conveyor_plugin_yourname.dll`

3. Load it in your pipeline:
   ```toml
   [global]
   plugins = ["yourname"]

   [[sources]]
   type = "yourname_source"
   # your config here
   ```

## Best Practices

1. **Error Handling**: Use `RResult` and `RBoxError` for all errors
2. **Validation**: Always validate configuration in `validate_config()`
3. **Testing**: Write unit tests for each module
4. **Documentation**: Document your configuration options
5. **Dependencies**: Minimize dependencies to reduce plugin size

## Examples

See working examples:
- `plugins/conveyor-plugin-test` - Minimal test plugin
- Future: `plugins/conveyor-plugin-http` - HTTP source/sink
- Future: `plugins/conveyor-plugin-mongodb` - MongoDB source/sink

## Troubleshooting

### Plugin not loading
- Ensure `_plugin_declaration` is exported with `#[no_mangle]`
- Check API version compatibility
- Verify the plugin file exists in the correct directory

### Build errors
- Ensure `conveyor-plugin-api` dependency path is correct
- Check that all FFI-safe types are used (RString, RVec, etc.)
- Use `rstr!()` macro for static string initialization

### Runtime errors
- Check configuration validation
- Ensure data formats match expected types
- Use proper error handling with `RBoxError`
