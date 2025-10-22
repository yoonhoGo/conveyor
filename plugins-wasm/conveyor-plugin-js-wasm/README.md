# JavaScript Inline Execution WASM Plugin

WebAssembly plugin for Conveyor that enables inline JavaScript code execution for data transformation using the Boa JavaScript engine (pure Rust implementation).

## Features

- **Inline JavaScript Execution**: Write transformation logic directly in the pipeline configuration
- **Row-by-Row Processing**: Transform each data row individually
- **Pure Rust Implementation**: Uses Boa engine (no external JavaScript runtime required)
- **Sandboxed Execution**: WASM provides memory isolation and security
- **Cross-Platform**: Same WASM binary works on all platforms

## Installation

The plugin is built as a WebAssembly module and loaded dynamically by Conveyor.

### Build

```bash
# Build the WASM plugin
cargo build -p conveyor-plugin-js-wasm --target wasm32-wasip2 --release

# Output: target/wasm32-wasip2/release/conveyor_plugin_js_wasm.wasm
```

## Usage

### Basic Configuration

```toml
[[stages]]
id = "js_transform"
function = "js.eval"
inputs = ["previous_stage"]

[stages.config]
script = '''
function transform(row) {
    // Your transformation logic here
    return {
        ...row,
        newField: row.field1 + row.field2
    };
}
'''
```

### Configuration Options

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `script` | String | ✅ Yes | JavaScript code defining `transform(row)` function |

### Script Requirements

The JavaScript code must define a `transform` function that:
- Takes a single `row` parameter (JavaScript object)
- Returns a JavaScript object (the transformed row)
- Is synchronous (no async/await support)

## Examples

### Example 1: Calculate Computed Fields

```toml
[[stages]]
id = "compute_fields"
function = "js.eval"
inputs = ["data"]

[stages.config]
script = '''
function transform(row) {
    return {
        ...row,
        fullName: row.firstName + ' ' + row.lastName,
        age: new Date().getFullYear() - row.birthYear,
        isAdult: (new Date().getFullYear() - row.birthYear) >= 18
    };
}
'''
```

**Input:**
```json
{
    "firstName": "Alice",
    "lastName": "Johnson",
    "birthYear": 1990
}
```

**Output:**
```json
{
    "firstName": "Alice",
    "lastName": "Johnson",
    "birthYear": 1990,
    "fullName": "Alice Johnson",
    "age": 35,
    "isAdult": true
}
```

### Example 2: Data Normalization

```toml
[[stages]]
id = "normalize"
function = "js.eval"
inputs = ["raw_data"]

[stages.config]
script = '''
function transform(row) {
    return {
        id: row.id,
        name: (row.name || '').trim().toLowerCase(),
        email: (row.email || '').trim().toLowerCase(),
        status: row.status === 'active' ? 1 : 0,
        createdAt: new Date(row.created_at).toISOString()
    };
}
'''
```

### Example 3: Conditional Logic

```toml
[[stages]]
id = "categorize"
function = "js.eval"
inputs = ["sales"]

[stages.config]
script = '''
function transform(row) {
    let category;
    if (row.amount > 10000) {
        category = 'high';
    } else if (row.amount > 1000) {
        category = 'medium';
    } else {
        category = 'low';
    }

    return {
        ...row,
        category: category,
        priority: category === 'high' ? 1 : 2
    };
}
'''
```

### Example 4: String Manipulation

```toml
[[stages]]
id = "format_text"
function = "js.eval"
inputs = ["text_data"]

[stages.config]
script = '''
function transform(row) {
    const text = row.content || '';
    return {
        ...row,
        wordCount: text.split(/\s+/).length,
        charCount: text.length,
        preview: text.substring(0, 100) + '...',
        uppercase: text.toUpperCase(),
        titleCase: text.split(' ')
            .map(word => word.charAt(0).toUpperCase() + word.slice(1))
            .join(' ')
    };
}
'''
```

## Complete Pipeline Example

See `examples/js-transform-example.toml` for a complete working example.

```toml
[pipeline]
name = "js-transform-example"
version = "1.0.0"

[global]
log_level = "info"

# Load data
[[stages]]
id = "load"
function = "json.read"
inputs = []
[stages.config]
path = "data/users.json"

# Transform with JavaScript
[[stages]]
id = "transform"
function = "js.eval"
inputs = ["load"]
[stages.config]
script = '''
function transform(row) {
    const age = new Date().getFullYear() - row.birthYear;
    return {
        ...row,
        fullName: row.firstName + ' ' + row.lastName,
        age: age,
        isAdult: age >= 18
    };
}
'''

# Display results
[[stages]]
id = "output"
function = "stdout.write"
inputs = ["transform"]
[stages.config]
format = "table"
```

Run:
```bash
conveyor run examples/js-transform-example.toml
```

## JavaScript API Support

The Boa engine supports most ECMAScript features:

### Supported Features
- ✅ Basic operators (+, -, *, /, %, etc.)
- ✅ String methods (substring, split, replace, etc.)
- ✅ Array methods (map, filter, reduce, etc.)
- ✅ Object manipulation (spread operator, destructuring)
- ✅ Conditional logic (if/else, ternary)
- ✅ Date objects (new Date(), getFullYear(), etc.)
- ✅ Math operations (Math.round, Math.floor, etc.)
- ✅ Regular expressions
- ✅ JSON methods (JSON.parse, JSON.stringify)

### Not Supported
- ❌ Async/await (synchronous only)
- ❌ Network requests (fetch, XMLHttpRequest)
- ❌ File I/O (no fs module)
- ❌ Node.js specific APIs
- ❌ Browser APIs (DOM, window, etc.)

## Performance

- **Execution Model**: Each row is processed individually
- **Overhead**: ~10-20% compared to native Rust transforms
- **Best For**: Complex transformation logic that would be verbose in Rust
- **Not Ideal For**: Simple operations (use built-in transforms instead)

## Error Handling

Errors in JavaScript code will cause the stage to fail:

- **Syntax errors**: Detected at parse time
- **Runtime errors**: Reported with row number
- **Missing transform function**: Clear error message

Example error:
```
JavaScript error at row 2: TypeError: Cannot read property 'length' of undefined
```

## Limitations

1. **No state between rows**: Each row is processed independently
2. **No external dependencies**: Pure JavaScript only, no npm packages
3. **Synchronous execution**: No async/await support
4. **Single input**: Only one input stage supported

## Best Practices

1. **Keep it simple**: Use for complex logic, not simple field mapping
2. **Test incrementally**: Start with simple transforms, add complexity gradually
3. **Handle nulls**: Check for null/undefined values before accessing properties
4. **Use built-ins first**: Prefer native transforms (filter, map) when possible
5. **Document your code**: Add comments to explain complex logic

## Troubleshooting

### "Missing required 'script' parameter"
- Ensure `script` field is present in `[stages.config]`

### "Failed to parse script"
- Check JavaScript syntax
- Ensure `transform` function is defined
- Use triple quotes (''') for multi-line scripts in TOML

### "Failed to execute transform function"
- Verify `transform` function exists and is callable
- Check function signature: `function transform(row) { ... }`

### "Failed to parse transformed result"
- Ensure transform function returns a valid object
- Use `JSON.stringify()` to debug return values

## Development

### Run Tests

```bash
cargo test -p conveyor-plugin-js-wasm
```

### Build for Release

```bash
cargo build -p conveyor-plugin-js-wasm --target wasm32-wasip2 --release
```

## License

MIT
