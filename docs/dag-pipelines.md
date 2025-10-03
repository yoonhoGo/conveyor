# DAG-Based Pipelines

Conveyor supports flexible DAG (Directed Acyclic Graph) based pipelines where you can compose stages in any order, with automatic dependency resolution and parallel execution.

## Why DAG Pipelines?

- **Flexible Composition**: Use sources, transforms, and sinks anywhere in the pipeline
- **Branching**: Send the same data to multiple stages (e.g., save to file AND display to console)
- **Sequential Chaining**: Source → Transform → HTTP Source (fetch related data) → Transform → Sink
- **Automatic Parallelization**: Independent stages execute concurrently
- **Cycle Detection**: Validates pipeline structure before execution

## DAG Pipeline Format

```toml
[pipeline]
name = "user-posts-pipeline"
version = "1.0"

[[stages]]
id = "load_users"           # Unique identifier
type = "source.json"        # Stage type: category.name
inputs = []                 # No inputs (this is a source)

[stages.config]
path = "users.json"

[[stages]]
id = "filter_active"
type = "transform.filter"
inputs = ["load_users"]     # Depends on load_users

[stages.config]
column = "status"
operator = "=="
value = "active"

# Branching: Same data goes to two different stages
[[stages]]
id = "save_to_file"
type = "sink.json"
inputs = ["filter_active"]

[stages.config]
path = "output.json"

[[stages]]
id = "display"
type = "sink.stdout"
inputs = ["filter_active"]  # Same input!

[stages.config]
format = "table"
```

## Stage Types

Stages use a `category.name` format:

- **Built-in Sources**: `source.csv`, `source.json`, `source.stdin`
- **Built-in Transforms**: `transform.filter`, `transform.map`, `transform.validate_schema`, `transform.http_fetch`
- **Built-in Sinks**: `sink.csv`, `sink.json`, `sink.stdout`
- **Plugin Stages**: `plugin.http`, `plugin.mongodb`, `wasm.echo`
- **Special Stages**: `stage.pipeline` (nested pipelines)

## Execution Model

### Topological Ordering

Stages execute in dependency order based on the `inputs` field. The DAG executor:

1. Builds a directed graph from stage dependencies
2. Performs topological sorting to determine execution order
3. Detects cycles and reports errors before execution

### Level-Based Parallelism

Stages are grouped into "levels" where:
- **Level 0**: Stages with no inputs (sources)
- **Level N**: Stages that depend only on stages in levels < N

All stages in the same level execute in parallel using Tokio tasks.

### Example Execution

```toml
[[stages]]
id = "source1"
type = "source.csv"
inputs = []

[[stages]]
id = "source2"
type = "source.json"
inputs = []

[[stages]]
id = "merge"
type = "transform.merge"
inputs = ["source1", "source2"]  # Depends on both sources

[[stages]]
id = "sink1"
type = "sink.json"
inputs = ["merge"]

[[stages]]
id = "sink2"
type = "sink.csv"
inputs = ["merge"]
```

**Execution levels:**
- **Level 0**: `source1` and `source2` (parallel)
- **Level 1**: `merge` (waits for level 0)
- **Level 2**: `sink1` and `sink2` (parallel)

## Branching Patterns

### Fan-Out: One Input, Multiple Outputs

```toml
[[stages]]
id = "data"
type = "source.csv"
inputs = []

# Branch 1: Save to JSON
[[stages]]
id = "save_json"
type = "sink.json"
inputs = ["data"]

# Branch 2: Save to CSV
[[stages]]
id = "save_csv"
type = "sink.csv"
inputs = ["data"]

# Branch 3: Display
[[stages]]
id = "display"
type = "sink.stdout"
inputs = ["data"]
```

All three sinks execute in parallel.

### Fan-In: Multiple Inputs, One Output

```toml
[[stages]]
id = "users"
type = "source.json"
inputs = []

[[stages]]
id = "orders"
type = "source.csv"
inputs = []

[[stages]]
id = "join"
type = "transform.join"
inputs = ["users", "orders"]

[stages.config]
join_type = "inner"
left_on = "user_id"
right_on = "user_id"
```

The join stage waits for both inputs to complete.

## Data Passing Between Stages

Data flows through stages via the executor's HashMap:

```rust
// Executor maintains a HashMap of stage outputs
outputs: HashMap<String, DataFormat>

// When a stage completes:
outputs.insert(stage_id, data);

// Next stages retrieve inputs:
for input_id in stage.inputs {
    let input_data = outputs.get(input_id)?;
    inputs.insert(input_id, input_data.clone());
}
```

### Multiple Inputs

When a stage has multiple inputs, they're passed as a HashMap:

```rust
// Stage with inputs = ["stage1", "stage2"]
inputs = {
    "stage1": DataFormat::DataFrame(df1),
    "stage2": DataFormat::DataFrame(df2),
}
```

The stage implementation decides how to combine them.

## Error Handling

Configure error handling strategy in the pipeline:

```toml
[error_handling]
strategy = "stop"  # stop, continue, retry
max_retries = 3
retry_delay_seconds = 5
```

When a stage fails:
- **stop**: Halt pipeline execution immediately
- **continue**: Skip failed stage, continue with empty DataFrame
- **retry**: Retry the stage up to `max_retries` times

## Validation

The DAG executor validates pipelines before execution:

### Cycle Detection

```toml
# This will fail validation!
[[stages]]
id = "stage1"
inputs = ["stage2"]

[[stages]]
id = "stage2"
inputs = ["stage1"]  # Cycle!
```

Error: `Cycle detected in pipeline at stage 'stage1'`

### Missing Inputs

```toml
[[stages]]
id = "sink"
inputs = ["nonexistent"]  # Error!
```

Error: `Stage 'sink' references non-existent input stage 'nonexistent'`

### Duplicate IDs

```toml
[[stages]]
id = "duplicate"
type = "source.csv"

[[stages]]
id = "duplicate"  # Error!
type = "source.json"
```

Error: `Duplicate stage id: 'duplicate'`

## Best Practices

1. **Use Descriptive IDs**: `load_users` instead of `stage1`
2. **Group Related Stages**: Use comments to separate logical sections
3. **Minimize Branching Depth**: Too many branches can be hard to follow
4. **Document Dependencies**: Add comments explaining why stages depend on each other
5. **Test in Isolation**: Validate each stage works independently before composing

## Examples

See the `examples/` directory for complete DAG pipeline examples:

- `dag-pipeline-example.toml` - Basic DAG with branching
- `http-chaining-example.toml` - Multi-step pipeline with HTTP fetch
- `http-batch-example.toml` - Batch API requests

## Migrating from Legacy Format

Legacy format (sequential):
```toml
[[sources]]
name = "data"
type = "csv"

[[transforms]]
name = "filter"
function = "filter"

[[sinks]]
name = "output"
type = "json"
```

DAG format (flexible):
```toml
[[stages]]
id = "data"
type = "source.csv"
inputs = []

[[stages]]
id = "filter"
type = "transform.filter"
inputs = ["data"]

[[stages]]
id = "output"
type = "sink.json"
inputs = ["filter"]
```

Key differences:
- `[[sources]]` → `[[stages]]` with `type = "source.csv"`
- `name` → `id`
- Add explicit `inputs = []` for sources
- Transforms use `type` instead of `function`
