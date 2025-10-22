# MongoDB Plugin

MongoDB database integration plugin for Conveyor. Provides comprehensive CRUD operations for MongoDB collections.

## Features

- **Complete CRUD** operations (Create, Read, Update, Delete)
- **Cursor-based pagination** for large datasets
- **Batch operations** for efficiency
- **Query filtering** with MongoDB query syntax
- **Connection pooling** for performance

## Installation

The MongoDB plugin is included with Conveyor. Enable it in your pipeline configuration:

```toml
[global]
plugins = ["mongodb"]
```

## Available Functions

### Read Operations (Sources)

- `mongodb.find` - Find multiple documents
- `mongodb.findOne` - Find single document

### Write Operations (Sinks)

- `mongodb.insertOne` - Insert single document
- `mongodb.insertMany` - Insert multiple documents
- `mongodb.updateOne` - Update single document
- `mongodb.updateMany` - Update multiple documents
- `mongodb.deleteOne` - Delete single document
- `mongodb.deleteMany` - Delete multiple documents
- `mongodb.replaceOne` - Replace single document
- `mongodb.replaceMany` - Replace multiple documents

## Configuration

### Common Options (All Operations)

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `uri` | String | ✅ Yes | - | MongoDB connection URI |
| `database` | String | ✅ Yes | - | Database name |
| `collection` | String | ✅ Yes | - | Collection name |

### Read Operations Options

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `query` | String | No | `{}` | MongoDB query (JSON string) |
| `limit` | Integer | No | - | Maximum documents to fetch |

### Write Operations Options

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `query` | String | ✅ Yes* | - | MongoDB query filter (for update/delete/replace) |

*Required for update, delete, and replace operations only.

## Connection URI Format

```
mongodb://[username:password@]host[:port][/database][?options]
```

**Examples:**

```toml
# Local MongoDB
uri = "mongodb://localhost:27017"

# Remote MongoDB with authentication
uri = "mongodb://user:password@mongodb.example.com:27017"

# MongoDB Atlas
uri = "mongodb+srv://user:password@cluster.mongodb.net/database"

# Using environment variable
uri = "${MONGODB_URI}"
```

## Read Operations

### mongodb.find

Find multiple documents matching a query.

**Example:**

```toml
[[stages]]
id = "load_users"
function = "mongodb.find"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "status": "active" }'
limit = 1000
```

**Advanced Query Examples:**

```toml
# Find by date range
query = '{ "created_at": { "$gte": "2024-01-01", "$lt": "2024-02-01" } }'

# Find with multiple conditions
query = '{ "status": "active", "age": { "$gte": 18 } }'

# Find with OR condition
query = '{ "$or": [{ "role": "admin" }, { "role": "moderator" }] }'

# Find all documents
query = '{}'
```

### mongodb.findOne

Find a single document.

**Example:**

```toml
[[stages]]
id = "load_user"
function = "mongodb.findOne"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "_id": "user123" }'
```

## Write Operations

### mongodb.insertOne

Insert a single document.

**Example:**

```toml
[[stages]]
id = "insert_user"
function = "mongodb.insertOne"
inputs = ["new_user_data"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
```

### mongodb.insertMany

Insert multiple documents (batch insert).

**Example:**

```toml
[[stages]]
id = "batch_insert"
function = "mongodb.insertMany"
inputs = ["processed_data"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "analytics"
collection = "events"
```

## Update Operations

### mongodb.updateOne

Update a single document matching the query.

**Example:**

```toml
[[stages]]
id = "update_status"
function = "mongodb.updateOne"
inputs = ["update_data"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "_id": "user123" }'
```

**Note:** The update data comes from the input stage. The first record's fields will be used as update values.

### mongodb.updateMany

Update all documents matching the query.

**Example:**

```toml
[[stages]]
id = "bulk_update"
function = "mongodb.updateMany"
inputs = ["updates"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "status": "pending" }'
```

## Delete Operations

### mongodb.deleteOne

Delete a single document matching the query.

**Example:**

```toml
[[stages]]
id = "delete_user"
function = "mongodb.deleteOne"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "_id": "user123" }'
```

### mongodb.deleteMany

Delete all documents matching the query.

**Example:**

```toml
[[stages]]
id = "cleanup_old"
function = "mongodb.deleteMany"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "logs"
query = '{ "created_at": { "$lt": "2024-01-01" } }'
```

## Replace Operations

### mongodb.replaceOne

Replace a single document.

**Example:**

```toml
[[stages]]
id = "replace_user"
function = "mongodb.replaceOne"
inputs = ["replacement_data"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "users"
query = '{ "_id": "user123" }'
```

### mongodb.replaceMany

Replace multiple documents.

**Example:**

```toml
[[stages]]
id = "bulk_replace"
function = "mongodb.replaceMany"
inputs = ["replacement_data"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "myapp"
collection = "cache"
query = '{ "expired": true }'
```

## Complete Examples

### Example 1: ETL Pipeline (MongoDB → Processing → MongoDB)

```toml
[pipeline]
name = "mongodb_etl"
version = "1.0.0"

[global]
plugins = ["mongodb"]
log_level = "info"

[global.variables]
mongo_uri = "${MONGODB_URI}"

# Load data from MongoDB
[[stages]]
id = "load_events"
function = "mongodb.find"
inputs = []

[stages.config]
uri = "{{mongo_uri}}"
database = "analytics"
collection = "raw_events"
query = '{ "processed": false }'
limit = 10000

# Process data
[[stages]]
id = "filter_valid"
function = "filter.apply"
inputs = ["load_events"]

[stages.config]
column = "status"
operator = "=="
value = "valid"

# Save processed data
[[stages]]
id = "save_processed"
function = "mongodb.insertMany"
inputs = ["filter_valid"]

[stages.config]
uri = "{{mongo_uri}}"
database = "analytics"
collection = "processed_events"

# Mark originals as processed
[[stages]]
id = "mark_processed"
function = "mongodb.updateMany"
inputs = ["filter_valid"]

[stages.config]
uri = "{{mongo_uri}}"
database = "analytics"
collection = "raw_events"
query = '{ "processed": false }'
```

### Example 2: Data Migration

```toml
[pipeline]
name = "migrate_users"
version = "1.0.0"

[global]
plugins = ["mongodb"]

# Load from old collection
[[stages]]
id = "load_old"
function = "mongodb.find"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "legacy_db"
collection = "users_old"

# Transform data
[[stages]]
id = "add_timestamp"
function = "map.apply"
inputs = ["load_old"]

[stages.config]
expression = "2024"
output_column = "migrated_year"

# Insert to new collection
[[stages]]
id = "save_new"
function = "mongodb.insertMany"
inputs = ["add_timestamp"]

[stages.config]
uri = "mongodb://localhost:27017"
database = "new_db"
collection = "users"
```

### Example 3: Cleanup Pipeline

```toml
[pipeline]
name = "cleanup_old_data"
version = "1.0.0"

[global]
plugins = ["mongodb"]

# Delete old logs (older than 30 days)
[[stages]]
id = "delete_old_logs"
function = "mongodb.deleteMany"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "app_logs"
collection = "access_logs"
query = '{ "timestamp": { "$lt": "2024-01-01" } }'

# Delete expired cache entries
[[stages]]
id = "delete_expired_cache"
function = "mongodb.deleteMany"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "app_cache"
collection = "cache_entries"
query = '{ "expires_at": { "$lt": "2024-02-01" } }'
```

### Example 4: MongoDB → CSV Export

```toml
[pipeline]
name = "export_to_csv"
version = "1.0.0"

[global]
plugins = ["mongodb"]

# Load from MongoDB
[[stages]]
id = "load_data"
function = "mongodb.find"
inputs = []

[stages.config]
uri = "mongodb://localhost:27017"
database = "analytics"
collection = "reports"
query = '{ "year": 2024 }'

# Save to CSV
[[stages]]
id = "export"
function = "csv.write"
inputs = ["load_data"]

[stages.config]
path = "reports_2024.csv"
has_headers = true
```

## Performance Tips

### 1. Use Indexes

Ensure queries use indexed fields for better performance:

```javascript
// In MongoDB shell
db.users.createIndex({ status: 1 })
db.events.createIndex({ created_at: -1 })
```

### 2. Limit Result Sets

Always use `limit` for large collections:

```toml
[stages.config]
limit = 10000  # Process in batches
```

### 3. Batch Operations

Use `insertMany` instead of multiple `insertOne` calls:

```toml
# Good: Single batch insert
function = "mongodb.insertMany"

# Bad: Multiple single inserts (slow)
# Don't create multiple insertOne stages
```

### 4. Connection Pooling

The plugin automatically manages connection pools. Reuse connections across stages by using the same URI.

## Error Handling

### Connection Errors

```toml
[error_handling]
strategy = "retry"
max_retries = 3
retry_delay_seconds = 5
```

### Common Errors

**Connection timeout:**
- Check MongoDB server is running
- Verify URI and credentials
- Check network connectivity

**Authentication failed:**
- Verify username and password
- Check database permissions
- Ensure user has required roles

**Query failed:**
- Validate query JSON syntax
- Check field names exist
- Verify query operators are valid

## Best Practices

1. **Use Environment Variables for Credentials**
   ```toml
   [global.variables]
   mongo_uri = "${MONGODB_URI}"
   ```

2. **Always Specify Limits**
   Prevent memory issues with large collections:
   ```toml
   limit = 10000
   ```

3. **Use Specific Queries**
   Avoid fetching unnecessary data:
   ```toml
   query = '{ "status": "active", "created_at": { "$gte": "2024-01-01" } }'
   ```

4. **Index Your Queries**
   Create indexes for frequently queried fields

5. **Batch Write Operations**
   Use `insertMany`, `updateMany` for efficiency

6. **Monitor Performance**
   - Enable debug logging during development
   - Track query execution times
   - Monitor MongoDB metrics

## MongoDB Query Language

### Comparison Operators

```javascript
{ "age": { "$eq": 25 } }      // Equal
{ "age": { "$ne": 25 } }      // Not equal
{ "age": { "$gt": 18 } }      // Greater than
{ "age": { "$gte": 18 } }     // Greater than or equal
{ "age": { "$lt": 65 } }      // Less than
{ "age": { "$lte": 65 } }     // Less than or equal
{ "role": { "$in": ["admin", "moderator"] } }  // In array
```

### Logical Operators

```javascript
{ "$and": [{ "age": { "$gte": 18 } }, { "status": "active" }] }
{ "$or": [{ "role": "admin" }, { "role": "moderator" }] }
{ "$not": { "status": "deleted" } }
```

### Array Operators

```javascript
{ "tags": { "$all": ["important", "urgent"] } }
{ "tags": { "$elemMatch": { "$eq": "featured" } } }
{ "items": { "$size": 3 } }
```

## Troubleshooting

### Plugin Not Loading

**Error:** `Plugin 'mongodb' not found`

**Solution:**
```toml
[global]
plugins = ["mongodb"]  # Ensure plugin is listed
```

### Invalid Query Syntax

**Error:** `Failed to parse query`

**Solution:**
- Validate JSON syntax
- Use single quotes for query string
- Escape special characters properly

```toml
# Correct
query = '{ "name": "John" }'

# Incorrect
query = "{ 'name': 'John' }"  # Wrong quotes
```

### Connection Pool Exhausted

**Error:** `Connection pool timeout`

**Solutions:**
- Reduce concurrent operations
- Increase connection pool size (in MongoDB server config)
- Add delays between batches

## See Also

- [Built-in Functions](../builtin-functions.md) - All built-in functions
- [Configuration Guide](../configuration.md) - Pipeline configuration
- [Plugin Development](../plugin-system.md) - Creating custom plugins
- [MongoDB Documentation](https://docs.mongodb.com/) - Official MongoDB docs
