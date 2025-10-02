use anyhow::Result;
use async_trait::async_trait;
use mongodb::{Client, bson::{doc, Document, Bson}};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::core::traits::{DataFormat, DataSource, RecordBatch};

pub struct MongodbSource;

#[async_trait]
impl DataSource for MongodbSource {
    async fn name(&self) -> &str {
        "mongodb"
    }

    async fn read(&self, config: &HashMap<String, toml::Value>) -> Result<DataFormat> {
        // Required fields
        let connection_string = config
            .get("connection_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("MongoDB source requires 'connection_string' configuration"))?;

        let database_name = config
            .get("database")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("MongoDB source requires 'database' configuration"))?;

        let collection_name = config
            .get("collection")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("MongoDB source requires 'collection' configuration"))?;

        // Optional fields
        let query_str = config
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");

        let projection_str = config
            .get("projection")
            .and_then(|v| v.as_str());

        let sort_str = config
            .get("sort")
            .and_then(|v| v.as_str());

        let limit = config
            .get("limit")
            .and_then(|v| v.as_integer())
            .map(|v| v as i64);

        let batch_size = config
            .get("batch_size")
            .and_then(|v| v.as_integer())
            .map(|v| v as u32)
            .unwrap_or(1000);

        let cursor_field = config
            .get("cursor_field")
            .and_then(|v| v.as_str());

        let cursor_value = config
            .get("cursor_value")
            .and_then(|v| v.as_str());

        // Parse query JSON
        let mut query_doc: Document = serde_json::from_str(query_str)
            .map_err(|e| anyhow::anyhow!("Invalid query JSON: {}", e))?;

        // Apply cursor-based pagination
        if let Some(field) = cursor_field {
            if let Some(value) = cursor_value {
                // Add cursor condition to query
                // Parse cursor value - could be ObjectId, String, Number, etc.
                let cursor_bson = parse_cursor_value(value)?;

                // Merge cursor condition with existing query
                query_doc.insert(field, doc! { "$gt": cursor_bson });

                tracing::debug!("Applied cursor pagination: field={}, value={}", field, value);
            }
        }

        // Connect to MongoDB
        tracing::info!("Connecting to MongoDB: {}", connection_string);
        let client = Client::with_uri_str(connection_string).await?;
        let database = client.database(database_name);
        let collection = database.collection::<Document>(collection_name);

        // Build find options
        let mut find_options = mongodb::options::FindOptions::default();
        find_options.batch_size = Some(batch_size);

        // Apply projection
        if let Some(proj_str) = projection_str {
            let projection_doc: Document = serde_json::from_str(proj_str)
                .map_err(|e| anyhow::anyhow!("Invalid projection JSON: {}", e))?;
            find_options.projection = Some(projection_doc);
        }

        // Apply sort
        if let Some(sort_str_val) = sort_str {
            let sort_doc: Document = serde_json::from_str(sort_str_val)
                .map_err(|e| anyhow::anyhow!("Invalid sort JSON: {}", e))?;
            find_options.sort = Some(sort_doc);
        } else if cursor_field.is_some() {
            // Auto-sort by cursor field if pagination is enabled
            let cursor_field_name = cursor_field.unwrap();
            find_options.sort = Some(doc! { cursor_field_name: 1 });
            tracing::debug!("Auto-applied sort by cursor field: {}", cursor_field_name);
        }

        // Apply limit
        if let Some(limit_val) = limit {
            find_options.limit = Some(limit_val);
        }

        // Execute query
        tracing::info!(
            "Querying MongoDB collection '{}': query={:?}, limit={:?}",
            collection_name,
            query_doc,
            limit
        );

        let mut cursor = collection.find(query_doc).with_options(find_options).await?;

        // Collect documents
        let mut records: RecordBatch = Vec::new();
        let mut count = 0;

        use futures::StreamExt;
        while let Some(result) = cursor.next().await {
            let doc = result?;
            let record = bson_to_json_map(&doc)?;
            records.push(record);
            count += 1;
        }

        tracing::info!("Retrieved {} documents from MongoDB", count);

        Ok(DataFormat::RecordBatch(records))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        // Check required fields
        if !config.contains_key("connection_string") {
            anyhow::bail!("MongoDB source requires 'connection_string' configuration");
        }

        if !config.contains_key("database") {
            anyhow::bail!("MongoDB source requires 'database' configuration");
        }

        if !config.contains_key("collection") {
            anyhow::bail!("MongoDB source requires 'collection' configuration");
        }

        // Validate query JSON if provided
        if let Some(query) = config.get("query") {
            if let Some(query_str) = query.as_str() {
                let _: Document = serde_json::from_str(query_str)
                    .map_err(|e| anyhow::anyhow!("Invalid query JSON: {}", e))?;
            } else {
                anyhow::bail!("Query must be a string");
            }
        }

        // Validate projection JSON if provided
        if let Some(projection) = config.get("projection") {
            if let Some(proj_str) = projection.as_str() {
                let _: Document = serde_json::from_str(proj_str)
                    .map_err(|e| anyhow::anyhow!("Invalid projection JSON: {}", e))?;
            } else {
                anyhow::bail!("Projection must be a string");
            }
        }

        // Validate sort JSON if provided
        if let Some(sort) = config.get("sort") {
            if let Some(sort_str) = sort.as_str() {
                let _: Document = serde_json::from_str(sort_str)
                    .map_err(|e| anyhow::anyhow!("Invalid sort JSON: {}", e))?;
            } else {
                anyhow::bail!("Sort must be a string");
            }
        }

        // Validate cursor_value is provided when cursor_field is set
        if config.contains_key("cursor_field") && !config.contains_key("cursor_value") {
            // This is valid - it's the first page
            tracing::debug!("cursor_field provided without cursor_value - will fetch first page");
        }

        Ok(())
    }
}

/// Convert BSON Document to HashMap<String, JsonValue>
fn bson_to_json_map(doc: &Document) -> Result<HashMap<String, JsonValue>> {
    let mut map = HashMap::new();

    for (key, value) in doc {
        let json_value = bson_to_json(value)?;
        map.insert(key.clone(), json_value);
    }

    Ok(map)
}

/// Convert BSON value to serde_json::Value
fn bson_to_json(bson: &Bson) -> Result<JsonValue> {
    let json = match bson {
        Bson::Double(v) => {
            serde_json::Number::from_f64(*v)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null)
        }
        Bson::String(v) => JsonValue::String(v.clone()),
        Bson::Array(arr) => {
            let json_arr: Result<Vec<JsonValue>> = arr.iter().map(bson_to_json).collect();
            JsonValue::Array(json_arr?)
        }
        Bson::Document(doc) => {
            let map = bson_to_json_map(doc)?;
            serde_json::to_value(map)?
        }
        Bson::Boolean(v) => JsonValue::Bool(*v),
        Bson::Null => JsonValue::Null,
        Bson::Int32(v) => JsonValue::Number((*v).into()),
        Bson::Int64(v) => JsonValue::Number((*v).into()),
        Bson::Timestamp(ts) => JsonValue::Number(ts.time.into()),
        Bson::DateTime(dt) => JsonValue::String(dt.to_string()),
        Bson::ObjectId(oid) => JsonValue::String(oid.to_hex()),
        Bson::Decimal128(d) => JsonValue::String(d.to_string()),
        Bson::RegularExpression(regex) => {
            JsonValue::String(format!("/{}/{}", regex.pattern, regex.options))
        }
        Bson::JavaScriptCode(code) => JsonValue::String(code.clone()),
        Bson::JavaScriptCodeWithScope(code_with_scope) => {
            JsonValue::String(code_with_scope.code.clone())
        }
        Bson::Symbol(s) => JsonValue::String(s.clone()),
        Bson::Undefined => JsonValue::Null,
        Bson::MaxKey => JsonValue::String("MaxKey".to_string()),
        Bson::MinKey => JsonValue::String("MinKey".to_string()),
        Bson::Binary(bin) => {
            // Convert binary to base64
            JsonValue::String(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &bin.bytes
            ))
        }
        Bson::DbPointer(_ptr) => JsonValue::String("DbPointer".to_string()),
    };

    Ok(json)
}

/// Parse cursor value string to BSON
fn parse_cursor_value(value: &str) -> Result<Bson> {
    // Try to parse as different types

    // Try ObjectId (24 hex characters)
    if value.len() == 24 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(oid) = mongodb::bson::oid::ObjectId::parse_str(value) {
            return Ok(Bson::ObjectId(oid));
        }
    }

    // Try as integer
    if let Ok(int_val) = value.parse::<i64>() {
        return Ok(Bson::Int64(int_val));
    }

    // Try as float
    if let Ok(float_val) = value.parse::<f64>() {
        return Ok(Bson::Double(float_val));
    }

    // Try as ISO 8601 datetime
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        // Convert chrono DateTime to SystemTime, then to bson DateTime
        let system_time: std::time::SystemTime = dt.into();
        return Ok(Bson::DateTime(system_time.into()));
    }

    // Default to string
    Ok(Bson::String(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mongodb_source_validation() {
        let source = MongodbSource;

        // Valid config
        let mut valid_config = HashMap::new();
        valid_config.insert("connection_string".to_string(), toml::Value::String("mongodb://localhost:27017".to_string()));
        valid_config.insert("database".to_string(), toml::Value::String("test".to_string()));
        valid_config.insert("collection".to_string(), toml::Value::String("users".to_string()));
        assert!(source.validate_config(&valid_config).await.is_ok());

        // Invalid config - missing connection_string
        let mut invalid_config = HashMap::new();
        invalid_config.insert("database".to_string(), toml::Value::String("test".to_string()));
        invalid_config.insert("collection".to_string(), toml::Value::String("users".to_string()));
        assert!(source.validate_config(&invalid_config).await.is_err());

        // Invalid config - invalid query JSON
        let mut invalid_query_config = HashMap::new();
        invalid_query_config.insert("connection_string".to_string(), toml::Value::String("mongodb://localhost:27017".to_string()));
        invalid_query_config.insert("database".to_string(), toml::Value::String("test".to_string()));
        invalid_query_config.insert("collection".to_string(), toml::Value::String("users".to_string()));
        invalid_query_config.insert("query".to_string(), toml::Value::String("not valid json".to_string()));
        assert!(source.validate_config(&invalid_query_config).await.is_err());
    }

    #[test]
    fn test_parse_cursor_value() {
        // Test ObjectId
        let oid_str = "507f1f77bcf86cd799439011";
        let result = parse_cursor_value(oid_str).unwrap();
        assert!(matches!(result, Bson::ObjectId(_)));

        // Test integer
        let int_str = "12345";
        let result = parse_cursor_value(int_str).unwrap();
        assert!(matches!(result, Bson::Int64(12345)));

        // Test float
        let float_str = "123.45";
        let result = parse_cursor_value(float_str).unwrap();
        assert!(matches!(result, Bson::Double(_)));

        // Test string
        let string_str = "some_value";
        let result = parse_cursor_value(string_str).unwrap();
        assert!(matches!(result, Bson::String(_)));
    }

    #[test]
    fn test_bson_to_json() {
        // Test basic types
        assert_eq!(bson_to_json(&Bson::String("test".to_string())).unwrap(), JsonValue::String("test".to_string()));
        assert_eq!(bson_to_json(&Bson::Int32(42)).unwrap(), JsonValue::Number(42.into()));
        assert_eq!(bson_to_json(&Bson::Boolean(true)).unwrap(), JsonValue::Bool(true));
        assert_eq!(bson_to_json(&Bson::Null).unwrap(), JsonValue::Null);
    }
}
