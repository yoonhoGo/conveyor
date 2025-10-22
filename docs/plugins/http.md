# HTTP Plugin

REST API integration plugin for Conveyor. Provides HTTP sources and sinks for fetching and sending data via HTTP/HTTPS.

## Features

- **GET, POST, PUT, PATCH, DELETE** methods
- **Custom headers** for authentication and content negotiation
- **Multiple data formats**: JSON, JSONL, raw bytes
- **Configurable timeouts** and retries
- **Source and Sink** modes (fetch data or send data)

## Installation

The HTTP plugin is included with Conveyor. Enable it in your pipeline configuration:

```toml
[global]
plugins = ["http"]
```

## Available Functions

### http.get

Fetch data from an HTTP endpoint (GET request).

### http.post

Send data to an HTTP endpoint (POST request).

### http.put

Update data via HTTP (PUT request).

### http.patch

Partially update data via HTTP (PATCH request).

### http.delete

Delete data via HTTP (DELETE request).

## Configuration

### Source Mode (Fetch Data)

Use HTTP plugin as a data source to fetch data from an API.

**Configuration Options:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `url` | String | ✅ Yes | - | API endpoint URL |
| `method` | String | No | `GET` | HTTP method |
| `format` | String | No | `json` | Response format: `json`, `jsonl`, `raw` |
| `headers` | Object | No | `{}` | Custom HTTP headers |
| `timeout_seconds` | Integer | No | `30` | Request timeout |

**Example:**

```toml
[global]
plugins = ["http"]

[[stages]]
id = "fetch_users"
function = "http.get"
inputs = []

[stages.config]
url = "https://api.example.com/users"
format = "json"
timeout_seconds = 30

[stages.config.headers]
Authorization = "Bearer ${API_TOKEN}"
Accept = "application/json"
```

### Sink Mode (Send Data)

Use HTTP plugin as a sink to send processed data to an API.

**Configuration Options:**

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `url` | String | ✅ Yes | - | API endpoint URL |
| `method` | String | No | `POST` | HTTP method |
| `format` | String | No | `json` | Request format: `json`, `jsonl`, `raw` |
| `headers` | Object | No | `{}` | Custom HTTP headers |
| `timeout_seconds` | Integer | No | `30` | Request timeout |

**Example:**

```toml
[[stages]]
id = "send_results"
function = "http.post"
inputs = ["processed_data"]

[stages.config]
url = "https://api.example.com/results"
format = "json"

[stages.config.headers]
Content-Type = "application/json"
X-API-Key = "${API_KEY}"
```

## Data Formats

### JSON (`json`)

Send/receive data as JSON object or array.

**Source:** Parses JSON response into DataFrame or RecordBatch.
**Sink:** Sends all data as JSON in request body.

```toml
[stages.config]
format = "json"
```

### JSONL (`jsonl`)

Newline-delimited JSON (one object per line).

**Source:** Parses each line as separate record.
**Sink:** Sends each record as separate JSON line.

```toml
[stages.config]
format = "jsonl"
```

### Raw (`raw`)

Raw bytes without parsing.

**Source:** Returns raw response body as bytes.
**Sink:** Sends data as-is without encoding.

```toml
[stages.config]
format = "raw"
```

## Authentication

### Bearer Token

```toml
[stages.config.headers]
Authorization = "Bearer ${API_TOKEN}"
```

### API Key (Header)

```toml
[stages.config.headers]
X-API-Key = "${API_KEY}"
```

### API Key (Query Parameter)

```toml
[stages.config]
url = "https://api.example.com/data?api_key=${API_KEY}"
```

### Basic Auth

```toml
[stages.config.headers]
Authorization = "Basic ${BASE64_CREDENTIALS}"
```

## Complete Examples

### Example 1: Fetch and Process API Data

```toml
[pipeline]
name = "api_pipeline"
version = "1.0.0"

[global]
plugins = ["http"]
log_level = "info"

# Fetch users from API
[[stages]]
id = "fetch_users"
function = "http.get"
inputs = []

[stages.config]
url = "https://jsonplaceholder.typicode.com/users"
format = "json"
timeout_seconds = 30

# Filter active users
[[stages]]
id = "filter_active"
function = "filter.apply"
inputs = ["fetch_users"]

[stages.config]
column = "id"
operator = ">="
value = 5

# Save to JSON file
[[stages]]
id = "save_results"
function = "json.write"
inputs = ["filter_active"]

[stages.config]
path = "active_users.json"
pretty = true
```

### Example 2: Process and Send to API

```toml
[pipeline]
name = "send_pipeline"
version = "1.0.0"

[global]
plugins = ["http"]

# Load data from CSV
[[stages]]
id = "load_data"
function = "csv.read"
inputs = []

[stages.config]
path = "sales.csv"

# Filter high-value sales
[[stages]]
id = "filter_high_value"
function = "filter.apply"
inputs = ["load_data"]

[stages.config]
column = "amount"
operator = ">="
value = 10000.0

# Send to webhook
[[stages]]
id = "send_webhook"
function = "http.post"
inputs = ["filter_high_value"]

[stages.config]
url = "https://hooks.slack.com/services/${SLACK_WEBHOOK_PATH}"
format = "json"

[stages.config.headers]
Content-Type = "application/json"
```

### Example 3: Multi-Step API Pipeline

```toml
[pipeline]
name = "multi_api_pipeline"
version = "1.0.0"

[global]
plugins = ["http"]
variables.base_url = "https://api.example.com"
variables.api_key = "${API_KEY}"

# Step 1: Fetch users
[[stages]]
id = "fetch_users"
function = "http.get"
inputs = []

[stages.config]
url = "{{base_url}}/users"
format = "json"

[stages.config.headers]
Authorization = "Bearer {{api_key}}"

# Step 2: Enrich with orders
[[stages]]
id = "enrich_orders"
function = "http.fetch"
inputs = ["fetch_users"]

[stages.config]
url = "{{base_url}}/users/{{ id }}/orders"
result_field = "orders"

[stages.config.headers]
Authorization = "Bearer {{api_key}}"

# Step 3: Send enriched data
[[stages]]
id = "send_enriched"
function = "http.post"
inputs = ["enrich_orders"]

[stages.config]
url = "{{base_url}}/analytics"
format = "json"

[stages.config.headers]
Authorization = "Bearer {{api_key}}"
Content-Type = "application/json"
```

## Error Handling

### Timeout Configuration

```toml
[stages.config]
timeout_seconds = 60  # Wait up to 60 seconds
```

### Retry Strategy

Configure at pipeline level:

```toml
[error_handling]
strategy = "retry"
max_retries = 3
retry_delay_seconds = 5
```

### HTTP Status Codes

- **2xx**: Success
- **4xx**: Client error (request issue, authentication failure)
- **5xx**: Server error (API unavailable, internal error)

Failed requests will trigger the configured error handling strategy.

## Best Practices

1. **Use Environment Variables for Secrets**
   ```toml
   [stages.config.headers]
   Authorization = "Bearer ${API_TOKEN}"
   ```

2. **Set Appropriate Timeouts**
   - Fast APIs: 5-10 seconds
   - Slow APIs: 30-60 seconds
   - Batch operations: 120+ seconds

3. **Handle Rate Limits**
   - Add delays between requests
   - Use retry strategy with backoff
   - Monitor API quotas

4. **Validate Responses**
   - Use `validate.schema` after HTTP source
   - Check required fields
   - Verify data types

5. **Monitor Performance**
   - Enable debug logging during development
   - Track request/response sizes
   - Monitor timeout failures

## Troubleshooting

### Connection Timeout

**Error:** `Request timed out`

**Solutions:**
- Increase `timeout_seconds`
- Check network connectivity
- Verify API availability

### Authentication Failure

**Error:** `401 Unauthorized`

**Solutions:**
- Verify API token is valid
- Check header format (Bearer, API-Key, etc.)
- Ensure environment variable is set

### Invalid Response Format

**Error:** `Failed to parse response`

**Solutions:**
- Check `format` matches actual response
- Inspect API response with debug logging
- Try `format = "raw"` to see raw response

### SSL/TLS Errors

**Error:** `SSL certificate verification failed`

**Solutions:**
- Verify HTTPS URL is correct
- Check if API uses valid SSL certificate
- Contact API provider for certificate issues

## See Also

- [Built-in Functions](../builtin-functions.md) - All built-in functions
- [HTTP Fetch Transform](../http-fetch-transform.md) - Per-row HTTP requests
- [Configuration Guide](../configuration.md) - Pipeline configuration
- [Plugin Development](../plugin-system.md) - Creating custom plugins
