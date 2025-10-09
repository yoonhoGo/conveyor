# Interactive Stage Builder - Usage Guide

The interactive stage builder (`conveyor build`) provides a guided, interpreter-style interface for creating pipeline stages with automatic validation and helpful prompts.

## Usage

```bash
conveyor build
```

## Features

### 1. Function Selection
- Lists all available functions grouped by category (Sources, Transforms, Sinks)
- Shows function names with their descriptions
- Validates that the selected function exists

### 2. Function Summary
- Displays detailed information about the selected function
- Shows description and purpose
- Indicates number of required and optional parameters

### 3. Stage Configuration
- **Stage ID**: Unique identifier for the stage in the pipeline
- **Inputs**: Comma-separated list of input stage IDs (empty for sources)

### 4. Parameter Collection
- **Required parameters**: Must provide values
- **Optional parameters**: Press Enter to use default value

For each parameter:
- Shows parameter name and type
- Displays description
- Shows validation rules (allowed values, min/max, etc.)
- Shows default value if available
- Validates input according to parameter type and rules

### 5. Validation
- **Type checking**: Ensures correct data types (string, integer, float, boolean, array)
- **Allowed values**: Validates against enumerated options
- **Numeric ranges**: Checks min/max constraints
- **String length**: Validates length constraints
- **Error messages**: Clear feedback when validation fails

### 6. Output
- Generates valid TOML configuration
- Displays the complete stage configuration
- Ready to copy into your pipeline file

## Example Session

```bash
$ conveyor build

======================================================================
Interactive Stage Builder
======================================================================

Available functions:
----------------------------------------------------------------------

Sources:
  • csv.read
  • json.read
  • stdin.read

Transforms:
  • filter.apply
  • map.apply
  • select.apply
  • sort.apply
  ...

Sinks:
  • csv.write
  • json.write
  • stdout.write

Enter function name: filter.apply

======================================================================
Function: filter.apply (transform)
======================================================================

Filter rows based on column values

Filters DataFrame rows using various comparison operators. Supports
numeric comparisons (>, >=, <, <=), equality checks (==, !=), string
operations (contains), and set membership (in). Uses Polars' lazy
evaluation for optimal performance.

2 required parameter(s), 1 optional parameter(s)

CONFIGURING STAGE PARAMETERS:
----------------------------------------------------------------------

Stage ID (unique identifier for this stage): filter_adults

Input stage IDs (comma-separated, or empty for sources): read_users

Required parameters:

column (string)
  Name of the column to filter on: age

value (string)
  Value to compare against (can be string, number, boolean, or array for 'in' operator): 18

Optional parameters (press Enter to use default):

operator (string) [default: ==]
  Comparison operator to use
  Allowed: ==, =, !=, <>, >, >=, <, <=, contains, in: >=

======================================================================
Generated Stage Configuration:
======================================================================

id = "filter_adults"
function = "filter.apply"
inputs = ["read_users"]

[config]
column = "age"
operator = ">="
value = 18
```

## Benefits

1. **No TOML syntax knowledge required**: Just answer questions
2. **Built-in validation**: Catches errors before running the pipeline
3. **Guided experience**: Shows all available options with descriptions
4. **Type safety**: Automatic type conversion and checking
5. **Example-driven**: Can see parameter descriptions and allowed values
6. **Fast workflow**: Quickly build complex stages interactively

## Tips

- Use `conveyor list` to explore available functions first
- Use `conveyor info <function>` to see detailed function documentation before building
- For array parameters, enter comma-separated values: `value1,value2,value3`
- For boolean parameters, use: `true`/`false`, `yes`/`no`, or `1`/`0`
- Press Ctrl+C to cancel at any time
- Copy the generated TOML into your pipeline configuration file

## Integration with Existing Workflows

The interactive builder complements existing commands:

```bash
# 1. Explore functions
conveyor list

# 2. Get detailed help
conveyor info filter.apply

# 3. Build stage interactively
conveyor build
# (generates TOML configuration)

# 4. Add to your pipeline file
# Copy the output into your pipeline.toml

# 5. Validate
conveyor validate -c pipeline.toml

# 6. Run
conveyor run -c pipeline.toml
```
