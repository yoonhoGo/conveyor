# Excel WASM Plugin for Conveyor

WebAssembly plugin for reading and writing Microsoft Excel files (.xlsx, .xls) in Conveyor pipelines.

## Features

- ✅ Read Excel files (.xlsx, .xls)
- ✅ Write Excel files (.xlsx)
- ✅ Multiple sheet support
- ✅ Header detection and handling
- ✅ All Excel data types (numbers, strings, booleans, dates)
- ✅ Cross-platform (WASM sandbox)
- ✅ Safe file system access

## Installation

This plugin is compiled as a WebAssembly module and loaded dynamically by Conveyor.

### Build

```bash
# Build the WASM plugin
cargo build -p conveyor-plugin-excel-wasm --target wasm32-wasip2 --release

# The output will be at:
# target/wasm32-wasip2/release/conveyor_plugin_excel_wasm.wasm
```

## Usage

### Load the Plugin

Add the plugin to your pipeline configuration:

```toml
[global]
wasm_plugins = ["excel_wasm"]
```

### Available Functions

#### `excel.read` - Read Excel Files (Source)

Reads an Excel file and converts it to data records.

**Configuration:**

```toml
[[stages]]
id = "read_data"
function = "excel.read"

[stages.config]
path = "data.xlsx"              # Required: Path to Excel file
sheet = "Sheet1"                # Optional: Sheet name or index (default: first sheet)
has_headers = true              # Optional: First row contains headers (default: true)
```

**Sheet Selection:**
- By name: `sheet = "Sales"`
- By index: `sheet = "0"` (zero-based)
- Default: First sheet in workbook

**Output:** JSON records with column names from headers

#### `excel.write` - Write Excel Files (Sink)

Writes data records to an Excel file.

**Configuration:**

```toml
[[stages]]
id = "write_output"
function = "excel.write"
inputs = ["previous_stage"]

[stages.config]
path = "output.xlsx"            # Required: Output file path
sheet = "Results"               # Optional: Sheet name (default: "Sheet1")
write_headers = true            # Optional: Write column headers (default: true)
```

**Features:**
- Automatically creates parent directories
- Overwrites existing files
- Preserves data types (numbers, strings, booleans)
- Sorts columns alphabetically for consistent output

## Examples

### Example 1: CSV to Excel

```toml
[pipeline]
name = "csv-to-excel"
description = "Convert CSV to Excel"

[global]
wasm_plugins = ["excel_wasm"]

[[stages]]
id = "read_csv"
function = "csv.read"
[stages.config]
path = "data.csv"

[[stages]]
id = "write_excel"
function = "excel.write"
inputs = ["read_csv"]
[stages.config]
path = "output.xlsx"
sheet = "Data"
```

### Example 2: Excel to Excel with Filtering

```toml
[pipeline]
name = "excel-filter"
description = "Read Excel, filter, write new Excel"

[global]
wasm_plugins = ["excel_wasm"]

[[stages]]
id = "read_excel"
function = "excel.read"
[stages.config]
path = "input.xlsx"
has_headers = true

[[stages]]
id = "filter_data"
function = "filter.apply"
inputs = ["read_excel"]
[stages.config]
column = "Amount"
operator = ">"
value = 1000

[[stages]]
id = "write_filtered"
function = "excel.write"
inputs = ["filter_data"]
[stages.config]
path = "filtered.xlsx"
sheet = "HighValue"
```

### Example 3: Multiple Sheet Processing

```toml
[[stages]]
id = "read_sales"
function = "excel.read"
[stages.config]
path = "report.xlsx"
sheet = "Sales"

[[stages]]
id = "read_expenses"
function = "excel.read"
[stages.config]
path = "report.xlsx"
sheet = "Expenses"
```

## Data Type Mapping

### Excel → JSON (Reading)

| Excel Type | JSON Type | Example |
|------------|-----------|---------|
| Number | Number | `123`, `45.67` |
| Text | String | `"Hello"` |
| Boolean | Boolean | `true`, `false` |
| Date/Time | String | `"2025-10-07 12:30:00"` |
| Empty | Null | `null` |
| Error | String | `"ERROR: #DIV/0!"` |

### JSON → Excel (Writing)

| JSON Type | Excel Type | Example |
|-----------|------------|---------|
| Number | Number | `123` → `123` |
| String | Text | `"Hello"` → `Hello` |
| Boolean | Boolean | `true` → `TRUE` |
| Null | Empty | `null` → (empty cell) |
| Object/Array | Text | `{"a":1}` → `"{\"a\":1}"` |

## Technical Details

### Dependencies

- **calamine 0.31**: Excel file reading (supports .xls and .xlsx)
- **rust_xlsxwriter 0.83**: Excel file writing (supports .xlsx only)
- **serde/serde_json**: Data serialization
- **wit-bindgen**: WASM Component Model bindings

### Architecture

- **Sandbox**: Runs in WASM sandbox with limited file system access
- **File Access**: Uses WASI preopened directories for security
- **Data Format**: Exchanges data as JSON records with host
- **Memory Safe**: No unsafe code, memory isolation from host

### Performance

- **Read**: Processes entire workbook into memory
- **Write**: Buffered write with in-memory workbook construction
- **File Size**: Suitable for files up to ~10MB (WASM memory limit)
- **Overhead**: ~5-10% vs native due to WASM serialization

## Limitations

- **Write Format**: Only .xlsx output (no .xls writing)
- **Formulas**: Not evaluated, stored as strings
- **Formatting**: Styles and formatting not preserved
- **Charts**: Charts and images not supported
- **Large Files**: Memory constrained by WASM (recommend < 10MB)
- **Streaming**: Not supported (loads entire file into memory)

## Error Handling

### Common Errors

**File Not Found:**
```
Error: Failed to open Excel file 'missing.xlsx': No such file or directory
```
→ Check file path and ensure file exists

**Invalid Sheet:**
```
Error: Sheet 'NonExistent' not found in workbook
```
→ Verify sheet name or use sheet index

**Permission Denied:**
```
Error: failed to find a pre-opened file descriptor
```
→ File path must be within current working directory or subdirectories

**Invalid Data:**
```
Error: Failed to parse input JSON: expected value at line 1
```
→ Input data must be valid JSON records

## Testing

Run the test suite:

```bash
# From project root
./target/debug/conveyor run --config examples/excel-test/csv-to-excel.toml
./target/debug/conveyor run --config examples/excel-test/excel-to-stdout.toml
./target/debug/conveyor run --config examples/excel-test/excel-roundtrip.toml
```

## Contributing

### Building from Source

1. Install Rust with `wasm32-wasip2` target:
   ```bash
   rustup target add wasm32-wasip2
   ```

2. Build the plugin:
   ```bash
   cargo build -p conveyor-plugin-excel-wasm --target wasm32-wasip2 --release
   ```

3. Test with Conveyor:
   ```bash
   cargo build --release
   ./target/release/conveyor run --config your-pipeline.toml
   ```

## License

MIT License - See LICENSE file for details

## Changelog

### v0.1.0 (2025-10-07)

- Initial release
- Excel read support (.xlsx, .xls)
- Excel write support (.xlsx)
- Multi-sheet support
- Header handling
- Complete data type mapping
