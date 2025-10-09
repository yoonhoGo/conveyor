# CLI Reference

Complete reference for all Conveyor CLI commands and options.

## Table of Contents

- [Global Options](#global-options)
- [Commands](#commands)
  - [run](#run)
  - [validate](#validate)
  - [list](#list)
  - [info](#info)
  - [build](#build)
  - [stage](#stage)
  - [update](#update)

## Global Options

Available for all commands:

```bash
-l, --log-level <LEVEL>    Set log level (trace, debug, info, warn, error)
-h, --help                 Print help information
-V, --version              Print version information
```

## Commands

### run

Run a pipeline from a TOML configuration file.

**Usage:**
```bash
conveyor run <CONFIG> [OPTIONS]
```

**Arguments:**
- `<CONFIG>` - Path to the TOML configuration file (required)

**Options:**
- `--dry-run` - Validate configuration without executing the pipeline

**Examples:**
```bash
# Run a pipeline
conveyor run pipeline.toml

# Validate only (dry run)
conveyor run pipeline.toml --dry-run

# Run with debug logging
conveyor run pipeline.toml --log-level debug
```

---

### validate

Validate a pipeline configuration without running it.

**Usage:**
```bash
conveyor validate <CONFIG>
```

**Arguments:**
- `<CONFIG>` - Path to the TOML configuration file (required)

**Examples:**
```bash
# Validate configuration
conveyor validate pipeline.toml

# Output on success:
# ✓ Configuration is valid
```

**Validation Checks:**
- TOML syntax validity
- Stage configuration completeness
- DAG cycle detection
- Parameter type checking
- Required fields presence

---

### list

List all available functions (sources, transforms, sinks).

**Usage:**
```bash
conveyor list [OPTIONS]
```

**Options:**
- `-t, --module-type <TYPE>` - Filter by type: `sources`, `transforms`, or `sinks`

**Examples:**
```bash
# List all functions with descriptions
conveyor list

# List only sources
conveyor list -t sources

# List only transforms
conveyor list -t transforms

# List only sinks
conveyor list -t sinks
```

**Output Format:**
```
Available Modules
======================================================================

SOURCES:
----------------------------------------------------------------------
  • csv.read                  - Read data from CSV files
  • json.read                 - Read data from JSON files
  • stdin.read                - Read data from standard input

TRANSFORMS:
----------------------------------------------------------------------
  • filter.apply              - Filter rows based on column values
  • map.apply                 - Apply mathematical expressions to create new columns
  • select.apply              - Select specific columns from DataFrame
  ...

SINKS:
----------------------------------------------------------------------
  • csv.write                 - Write data to CSV files
  • json.write                - Write data to JSON files
  • stdout.write              - Write data to standard output
```

---

### info

Show detailed information about a specific function.

**Usage:**
```bash
conveyor info <FUNCTION>
```

**Arguments:**
- `<FUNCTION>` - Function name (e.g., `csv.read`, `filter.apply`)

**Examples:**
```bash
# Get detailed information
conveyor info csv.read
conveyor info filter.apply
conveyor info json.write
```

**Output Includes:**
- Function name and category
- Short and long descriptions
- Required parameters (with types and descriptions)
- Optional parameters (with types, defaults, and descriptions)
- Validation rules (allowed values, ranges, constraints)
- Usage examples with sample configurations
- Tags for categorization

**Example Output:**
```
======================================================================
Function: filter.apply
Category: transform
======================================================================

Filter rows based on column values

Filters DataFrame rows using various comparison operators. Supports
numeric comparisons (>, >=, <, <=), equality checks (==, !=), string
operations (contains), and set membership (in).

PARAMETERS:
----------------------------------------------------------------------

Required:
  • column (string)
    Name of the column to filter on

  • value (string)
    Value to compare against

Optional:
  • operator (string) [default: ==]
    Comparison operator to use
    Allowed values: ==, =, !=, <>, >, >=, <, <=, contains, in

EXAMPLES:
----------------------------------------------------------------------

1. Filter adults (age >= 18)
   Keep only rows where age is 18 or greater

   Config:
     column = "age"
     operator = ">="
     value = 18

TAGS:
----------------------------------------------------------------------
  filter, transform, dataframe
```

---

### build

Build a stage interactively with guided prompts.

**Usage:**
```bash
conveyor build
```

**Interactive Flow:**

1. **Function Selection**
   - Displays all available functions grouped by category
   - Prompts for function name

2. **Function Summary**
   - Shows selected function's description
   - Displays parameter count

3. **Stage Configuration**
   - Stage ID (unique identifier)
   - Input stage IDs (comma-separated)

4. **Parameter Collection**
   - Required parameters (must provide)
   - Optional parameters (press Enter for default)
   - Real-time validation
   - Clear error messages

5. **Output Generation**
   - Displays generated TOML configuration
   - Ready to copy into pipeline file

**Features:**
- Type validation (string, integer, float, boolean, array)
- Enum validation (allowed values)
- Range validation (min/max)
- Length validation (min_length/max_length)
- Default value support
- Helpful error messages

**Example Session:**
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

Transforms:
  • filter.apply
  • map.apply

Sinks:
  • json.write
  • stdout.write

Enter function name: filter.apply

======================================================================
Function: filter.apply (transform)
======================================================================

Filter rows based on column values
...

Stage ID (unique identifier for this stage): filter_adults

Input stage IDs (comma-separated, or empty for sources): read_users

Required parameters:

column (string)
  Name of the column to filter on: age

value (string)
  Value to compare against: 18

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

---

### stage

Stage management commands.

**Usage:**
```bash
conveyor stage <SUBCOMMAND>
```

#### stage new

Create a new DAG-based pipeline template.

**Usage:**
```bash
conveyor stage new [OPTIONS]
```

**Options:**
- `-o, --output <FILE>` - Output file path (default: `pipeline.toml`)
- `-i, --interactive` - Interactive mode with prompts

**Examples:**
```bash
# Create default template
conveyor stage new

# Specify output file
conveyor stage new -o my-pipeline.toml

# Interactive mode
conveyor stage new -i
```

#### stage add

Add a new stage to an existing pipeline.

**Usage:**
```bash
conveyor stage add <PIPELINE>
```

**Arguments:**
- `<PIPELINE>` - Path to the pipeline TOML file

**Examples:**
```bash
conveyor stage add pipeline.toml
```

#### stage edit

Edit pipeline stages interactively.

**Usage:**
```bash
conveyor stage edit <PIPELINE>
```

**Arguments:**
- `<PIPELINE>` - Path to the pipeline TOML file

**Examples:**
```bash
conveyor stage edit pipeline.toml
```

#### stage describe

Export function metadata in JSON format.

**Usage:**
```bash
conveyor stage describe <FUNCTION>
```

**Arguments:**
- `<FUNCTION>` - Function name

**Examples:**
```bash
# Export metadata as JSON
conveyor stage describe csv.read

# Pipe to jq for processing
conveyor stage describe filter.apply | jq '.parameters'

# Save to file
conveyor stage describe json.write > metadata.json
```

**Output Format:**
```json
{
  "name": "csv.read",
  "category": "Source",
  "description": "Read data from CSV files",
  "long_description": "Reads CSV files and converts them into a DataFrame...",
  "parameters": [
    {
      "name": "path",
      "param_type": "String",
      "required": true,
      "default": null,
      "description": "Path to the CSV file to read",
      "validation": null
    },
    ...
  ],
  "examples": [...],
  "tags": ["csv", "file", "io", "source"]
}
```

---

### update

Update Conveyor to the latest version.

**Usage:**
```bash
conveyor update
```

**Examples:**
```bash
# Check for updates and install latest version
conveyor update
```

---

## Common Workflows

### Discovery and Learning

```bash
# 1. List all available functions
conveyor list

# 2. Get details about a specific function
conveyor info csv.read

# 3. Build a stage interactively
conveyor build
```

### Development Workflow

```bash
# 1. Create a new pipeline
conveyor stage new -o pipeline.toml

# 2. Edit and add stages
# (edit pipeline.toml manually or use conveyor build)

# 3. Validate configuration
conveyor validate pipeline.toml

# 4. Test run
conveyor run pipeline.toml --dry-run

# 5. Execute pipeline
conveyor run pipeline.toml
```

### Debugging

```bash
# Run with debug logging
conveyor run pipeline.toml --log-level debug

# Validate before running
conveyor validate pipeline.toml

# Dry run to check configuration
conveyor run pipeline.toml --dry-run
```

---

## Environment Variables

Conveyor respects the following environment variables:

- `RUST_LOG` - Alternative to `--log-level` flag
- `NO_COLOR` - Disable colored output
- API keys for AI transforms:
  - `OPENAI_API_KEY`
  - `ANTHROPIC_API_KEY`
  - `OPENROUTER_API_KEY`

**Examples:**
```bash
# Set log level via environment
export RUST_LOG=debug
conveyor run pipeline.toml

# Disable colors
export NO_COLOR=1
conveyor list
```

---

## Exit Codes

- `0` - Success
- `1` - General error (invalid configuration, execution failure, etc.)

---

## Tips

1. **Use `info` before building**: Check function details with `conveyor info` before using `conveyor build` to understand what parameters you'll need.

2. **Validate early and often**: Run `conveyor validate` frequently during development to catch errors early.

3. **Export metadata for tooling**: Use `conveyor stage describe` to integrate Conveyor metadata into your own tools and scripts.

4. **Filter lists for focus**: Use `conveyor list -t sources` to focus on specific module types when exploring.

5. **Interactive building for complex stages**: Use `conveyor build` for stages with many parameters or complex validation rules.
