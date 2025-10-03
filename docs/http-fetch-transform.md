# HTTP Fetch Transform

The `http_fetch` transform enables dynamic HTTP API calls within your pipeline, using previous stage data as context.

## Key Features

- **Template-based URLs**: Use Handlebars templates to create dynamic URLs from row data
- **Per-row Mode**: Make individual API calls for each data row
- **Batch Mode**: Send all data in a single API request
- **Custom Headers**: Add authentication and custom headers
- **Error Handling**: Gracefully handles failed requests with null values

## Basic Usage

### Per-Row Mode (Default)

Make an API call for each row in your dataset:

```toml
[[stages]]
id = "load_users"
type = "source.json"
inputs = []

[stages.config]
path = "users.json"

[[stages]]
id = "fetch_posts"
type = "transform.http_fetch"
inputs = ["load_users"]

[stages.config]
url = "https://api.example.com/users/{{ id }}/posts"
method = "GET"
mode = "per_row"  # Default
result_field = "posts"
```

**Input data:**
```json
[
  {"id": 1, "name": "Alice"},
  {"id": 2, "name": "Bob"}
]
```

**Output data:**
```json
[
  {"id": 1, "name": "Alice", "posts": [...]},
  {"id": 2, "name": "Bob", "posts": [...]}
]
```

### Batch Mode

Send all data in a single API request:

```toml
[[stages]]
id = "batch_request"
type = "transform.http_fetch"
inputs = ["data"]

[stages.config]
url = "https://api.example.com/batch"
method = "POST"
mode = "batch"
result_field = "api_response"

# Template has access to all records via {{ records }}
body = '''
{
  "user_ids": [
    {{#each records}}
    {{ this.id }}{{#unless @last}},{{/unless}}
    {{/each}}
  ]
}
'''
```

**Input data:**
```json
[
  {"id": 1, "name": "Alice"},
  {"id": 2, "name": "Bob"}
]
```

**Request body:**
```json
{
  "user_ids": [1, 2]
}
```

## Configuration Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `url` | ✅ Yes | - | URL template (supports `{{ field }}` syntax) |
| `method` | No | `GET` | HTTP method (GET, POST, PUT, PATCH, DELETE) |
| `mode` | No | `per_row` | `per_row` (N calls) or `batch` (1 call) |
| `result_field` | No | `http_result` | Field name for storing API response |
| `body` | No | - | Request body template (for POST/PUT/PATCH) |
| `headers` | No | - | Custom HTTP headers |
| `timeout` | No | 30 | Request timeout in seconds |

## Template Syntax

Conveyor uses [Handlebars](https://handlebarsjs.com/) for templating.

### Per-Row Templates

Access current row fields with `{{ field_name }}`:

```toml
url = "https://api.example.com/users/{{ user_id }}/posts/{{ post_id }}"
```

For nested fields, use dot notation:
```toml
url = "https://api.example.com/{{ user.id }}/{{ user.profile.type }}"
```

### Batch Templates

Access all records with `{{ records }}`:

```toml
body = '''
{
  "items": [
    {{#each records}}
    {
      "id": {{ this.id }},
      "name": "{{ this.name }}"
    }{{#unless @last}},{{/unless}}
    {{/each}}
  ]
}
'''
```

Handlebars helpers:
- `{{#each}}...{{/each}}`: Loop over records
- `{{#unless @last}},{{/unless}}`: Add comma between items
- `{{#if}}...{{/if}}`: Conditional logic

## Authentication

### Bearer Token

```toml
[stages.config]
url = "https://api.example.com/data"

[stages.config.headers]
Authorization = "Bearer YOUR_API_TOKEN"
```

### API Key

```toml
[stages.config]
url = "https://api.example.com/data"

[stages.config.headers]
X-API-Key = "YOUR_API_KEY"
```

### Dynamic Authentication

Use row data for authentication:

```toml
url = "https://api.example.com/data"

[stages.config.headers]
Authorization = "Bearer {{ user_token }}"
```

## Error Handling

### Per-Row Mode

Failed requests return `null` for that row:

```json
[
  {"id": 1, "name": "Alice", "posts": [...]},
  {"id": 2, "name": "Bob", "posts": null},  // Request failed
  {"id": 3, "name": "Charlie", "posts": [...]}
]
```

### Batch Mode

Failed batch requests fail the entire stage (respects error handling strategy).

### Timeout Configuration

```toml
[stages.config]
url = "https://api.example.com/slow-endpoint"
timeout = 60  # Wait up to 60 seconds
```

## Complete Examples

### Example 1: Enrich User Data

```toml
[pipeline]
name = "enrich-users"
version = "1.0"

# Load users from CSV
[[stages]]
id = "load_users"
type = "source.csv"
inputs = []

[stages.config]
path = "users.csv"

# Fetch profile data for each user
[[stages]]
id = "fetch_profiles"
type = "transform.http_fetch"
inputs = ["load_users"]

[stages.config]
url = "https://api.example.com/profiles/{{ user_id }}"
method = "GET"
result_field = "profile"
timeout = 10

[stages.config.headers]
Authorization = "Bearer sk_live_..."

# Save enriched data
[[stages]]
id = "save"
type = "sink.json"
inputs = ["fetch_profiles"]

[stages.config]
path = "enriched_users.json"
```

### Example 2: Multi-Step Pipeline

```toml
# Step 1: Load users
[[stages]]
id = "users"
type = "source.json"
inputs = []

[stages.config]
path = "users.json"

# Step 2: Filter active users
[[stages]]
id = "active"
type = "transform.filter"
inputs = ["users"]

[stages.config]
column = "status"
operator = "=="
value = "active"

# Step 3: Fetch posts for active users
[[stages]]
id = "fetch_posts"
type = "transform.http_fetch"
inputs = ["active"]

[stages.config]
url = "https://jsonplaceholder.typicode.com/users/{{ id }}/posts"
result_field = "posts"

# Step 4: Filter users with posts
[[stages]]
id = "with_posts"
type = "transform.filter"
inputs = ["fetch_posts"]

[stages.config]
column = "posts"
operator = "!="
value = null

# Step 5: Save final results
[[stages]]
id = "save"
type = "sink.json"
inputs = ["with_posts"]

[stages.config]
path = "users_with_posts.json"
```

### Example 3: Batch API Call

```toml
[[stages]]
id = "orders"
type = "source.csv"
inputs = []

[stages.config]
path = "orders.csv"

[[stages]]
id = "validate_batch"
type = "transform.http_fetch"
inputs = ["orders"]

[stages.config]
url = "https://api.example.com/orders/validate"
method = "POST"
mode = "batch"
result_field = "validation_results"

body = '''
{
  "orders": [
    {{#each records}}
    {
      "order_id": "{{ this.order_id }}",
      "amount": {{ this.amount }}
    }{{#unless @last}},{{/unless}}
    {{/each}}
  ]
}
'''

[stages.config.headers]
Content-Type = "application/json"
X-API-Key = "{{ env.API_KEY }}"
```

## Use Cases

1. **Data Enrichment**: Add related information from APIs
   - User profiles, product details, geolocation data

2. **Validation**: Check data against external services
   - Email verification, address validation, fraud detection

3. **Multi-step Pipelines**: Chain data transformations
   - Load → Filter → API → Transform → Save

4. **Data Aggregation**: Collect data from multiple endpoints
   - Fetch posts, comments, and likes for each user

5. **Webhook Notifications**: Send data to external services
   - Slack notifications, webhook triggers

## Best Practices

1. **Rate Limiting**: Use `per_row` mode with caution for large datasets
   - Consider batch mode or add delays between requests

2. **Error Handling**: Set appropriate error strategy
   ```toml
   [error_handling]
   strategy = "continue"  # Don't fail entire pipeline on API errors
   ```

3. **Timeouts**: Configure realistic timeouts
   - Too short: False failures
   - Too long: Pipeline hangs

4. **Authentication**: Store secrets in environment variables
   ```toml
   [stages.config.headers]
   Authorization = "Bearer {{ env.API_TOKEN }}"
   ```

5. **Data Size**: Be mindful of response sizes
   - Large responses can impact memory usage
   - Consider pagination for large datasets

## Limitations

- **No async batching**: Per-row requests are sequential (may be slow for large datasets)
- **No pagination support**: Must handle paginated APIs in multiple stages
- **Template-only**: No complex expressions (use transforms for data prep)

## See Also

- [DAG Pipelines](dag-pipelines.md) - Learn about pipeline composition
- [Modules Reference](modules-reference.md) - All available transforms
- [Configuration](configuration.md) - Pipeline configuration options
