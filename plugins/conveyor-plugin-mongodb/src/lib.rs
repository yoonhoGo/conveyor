//! MongoDB Plugin for Conveyor
//!
//! Provides FFI-safe MongoDB source and sink capabilities for database integration.

use conveyor_plugin_api::{
    data::FfiDataFormat,
    rstr,
    traits::{FfiDataSource, FfiSink},
    PluginDeclaration, RBoxError, RHashMap, RResult, RStr, RString, ROk, RErr,
};
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, FindOptions},
    Client,
};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// MongoDB Source Implementation
// ============================================================================

/// MongoDB Source - reads data from MongoDB collections
struct MongoDbSource;

impl FfiDataSource for MongoDbSource {
    fn name(&self) -> RStr<'_> {
        "mongodb_source".into()
    }

    fn read(&self, config: RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to create runtime: {}", e))),
        };

        runtime.block_on(async { self.read_async(&config).await })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        if !config.contains_key(&RString::from("uri")) {
            return RErr(RBoxError::from_fmt(&format_args!("MongoDB source requires 'uri' configuration")));
        }

        if !config.contains_key(&RString::from("database")) {
            return RErr(RBoxError::from_fmt(&format_args!("MongoDB source requires 'database' configuration")));
        }

        if !config.contains_key(&RString::from("collection")) {
            return RErr(RBoxError::from_fmt(&format_args!("MongoDB source requires 'collection' configuration")));
        }

        ROk(())
    }
}

impl MongoDbSource {
    async fn read_async(&self, config: &RHashMap<RString, RString>) -> RResult<FfiDataFormat, RBoxError> {
        // Get MongoDB URI
        let uri = match config.get(&RString::from("uri")) {
            Some(u) => u.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'uri' configuration"))),
        };

        // Get database name
        let database = match config.get(&RString::from("database")) {
            Some(d) => d.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'database' configuration"))),
        };

        // Get collection name
        let collection_name = match config.get(&RString::from("collection")) {
            Some(c) => c.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'collection' configuration"))),
        };

        // Parse limit (default: no limit)
        let limit: Option<i64> = config
            .get(&RString::from("limit"))
            .and_then(|l| l.as_str().parse().ok());

        // Connect to MongoDB
        let client_options = match ClientOptions::parse(uri).await {
            Ok(opts) => opts,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to parse MongoDB URI: {}", e))),
        };

        let client = match Client::with_options(client_options) {
            Ok(c) => c,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to create MongoDB client: {}", e))),
        };

        let db = client.database(database);
        let collection = db.collection::<Document>(collection_name);

        // Build query filter (optional)
        let filter = if let Some(query_str) = config.get(&RString::from("query")) {
            match serde_json::from_str::<Value>(query_str.as_str()) {
                Ok(Value::Object(obj)) => {
                    let mut filter_doc = Document::new();
                    for (key, value) in obj {
                        if let Some(bson_val) = json_to_bson(&value) {
                            filter_doc.insert(key, bson_val);
                        }
                    }
                    filter_doc
                }
                _ => Document::new(),
            }
        } else {
            Document::new()
        };

        // Build find options
        let mut find_options = FindOptions::default();
        if let Some(lim) = limit {
            find_options.limit = Some(lim);
        }

        // Execute query
        let mut cursor = match collection.find(filter).with_options(find_options).await {
            Ok(c) => c,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("MongoDB query failed: {}", e))),
        };

        // Collect results
        let mut records: Vec<HashMap<String, Value>> = Vec::new();

        use futures::stream::TryStreamExt;
        while let Some(doc) = match cursor.try_next().await {
            Ok(d) => d,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to fetch document: {}", e))),
        } {
            if let Ok(json_str) = serde_json::to_string(&doc) {
                if let Ok(json_val) = serde_json::from_str::<Value>(&json_str) {
                    if let Some(obj) = json_val.as_object() {
                        let record: HashMap<String, Value> = obj
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        records.push(record);
                    }
                }
            }
        }

        FfiDataFormat::from_json_records(&records)
    }
}

// Helper function to convert JSON value to BSON
fn json_to_bson(value: &Value) -> Option<mongodb::bson::Bson> {
    match value {
        Value::Null => Some(mongodb::bson::Bson::Null),
        Value::Bool(b) => Some(mongodb::bson::Bson::Boolean(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(mongodb::bson::Bson::Int64(i))
            } else if let Some(f) = n.as_f64() {
                Some(mongodb::bson::Bson::Double(f))
            } else {
                None
            }
        }
        Value::String(s) => Some(mongodb::bson::Bson::String(s.clone())),
        Value::Array(arr) => {
            let bson_arr: Vec<_> = arr.iter().filter_map(json_to_bson).collect();
            Some(mongodb::bson::Bson::Array(bson_arr))
        }
        Value::Object(obj) => {
            let mut doc = Document::new();
            for (k, v) in obj {
                if let Some(bson_val) = json_to_bson(v) {
                    doc.insert(k.clone(), bson_val);
                }
            }
            Some(mongodb::bson::Bson::Document(doc))
        }
    }
}

// ============================================================================
// MongoDB Sink Implementation
// ============================================================================

/// MongoDB Sink - writes data to MongoDB collections
struct MongoDbSink;

impl FfiSink for MongoDbSink {
    fn name(&self) -> RStr<'_> {
        "mongodb_sink".into()
    }

    fn write(&self, data: FfiDataFormat, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to create runtime: {}", e))),
        };

        runtime.block_on(async { self.write_async(data, &config).await })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        if !config.contains_key(&RString::from("uri")) {
            return RErr(RBoxError::from_fmt(&format_args!("MongoDB sink requires 'uri' configuration")));
        }

        if !config.contains_key(&RString::from("database")) {
            return RErr(RBoxError::from_fmt(&format_args!("MongoDB sink requires 'database' configuration")));
        }

        if !config.contains_key(&RString::from("collection")) {
            return RErr(RBoxError::from_fmt(&format_args!("MongoDB sink requires 'collection' configuration")));
        }

        ROk(())
    }
}

impl MongoDbSink {
    async fn write_async(&self, data: FfiDataFormat, config: &RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Get MongoDB URI
        let uri = match config.get(&RString::from("uri")) {
            Some(u) => u.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'uri' configuration"))),
        };

        // Get database name
        let database = match config.get(&RString::from("database")) {
            Some(d) => d.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'database' configuration"))),
        };

        // Get collection name
        let collection_name = match config.get(&RString::from("collection")) {
            Some(c) => c.as_str(),
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing 'collection' configuration"))),
        };

        // Connect to MongoDB
        let client_options = match ClientOptions::parse(uri).await {
            Ok(opts) => opts,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to parse MongoDB URI: {}", e))),
        };

        let client = match Client::with_options(client_options) {
            Ok(c) => c,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to create MongoDB client: {}", e))),
        };

        let db = client.database(database);
        let collection = db.collection::<Document>(collection_name);

        // Convert data to JSON records
        let records = match data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        // Convert records to BSON documents
        let documents: Vec<Document> = records
            .iter()
            .filter_map(|record| {
                let mut doc = Document::new();
                for (key, value) in record {
                    if let Some(bson_val) = json_to_bson(value) {
                        doc.insert(key.clone(), bson_val);
                    }
                }
                if doc.is_empty() {
                    None
                } else {
                    Some(doc)
                }
            })
            .collect();

        if documents.is_empty() {
            return ROk(());
        }

        // Insert documents
        match collection.insert_many(documents).await {
            Ok(_) => ROk(()),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!("MongoDB insert failed: {}", e))),
        }
    }
}

// ============================================================================
// Plugin Registration
// ============================================================================

extern "C" fn register() -> RResult<(), RBoxError> {
    // Future: Register modules with host registry
    ROk(())
}

/// Plugin declaration - exported symbol
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: conveyor_plugin_api::PLUGIN_API_VERSION,
    name: rstr!("mongodb"),
    version: rstr!("0.1.0"),
    description: rstr!("MongoDB plugin for database integration"),
    register,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mongodb_source() {
        let source = MongoDbSource;
        assert_eq!(source.name(), "mongodb_source");
    }

    #[test]
    fn test_mongodb_sink() {
        let sink = MongoDbSink;
        assert_eq!(sink.name(), "mongodb_sink");
    }

    #[test]
    fn test_plugin_declaration() {
        assert_eq!(_plugin_declaration.name, "mongodb");
        assert_eq!(_plugin_declaration.version, "0.1.0");
        assert!(_plugin_declaration.is_compatible());
    }

    #[test]
    fn test_source_validation() {
        let source = MongoDbSource;
        let mut config = RHashMap::new();

        // Missing all configs should fail
        assert!(source.validate_config(config.clone()).is_err());

        // With only URI should still fail
        config.insert(RString::from("uri"), RString::from("mongodb://localhost:27017"));
        assert!(source.validate_config(config.clone()).is_err());

        // With URI and database should still fail
        config.insert(RString::from("database"), RString::from("testdb"));
        assert!(source.validate_config(config.clone()).is_err());

        // With all required configs should succeed
        config.insert(RString::from("collection"), RString::from("testcol"));
        assert!(source.validate_config(config).is_ok());
    }

    #[test]
    fn test_sink_validation() {
        let sink = MongoDbSink;
        let mut config = RHashMap::new();

        // Missing all configs should fail
        assert!(sink.validate_config(config.clone()).is_err());

        // With only URI should still fail
        config.insert(RString::from("uri"), RString::from("mongodb://localhost:27017"));
        assert!(sink.validate_config(config.clone()).is_err());

        // With URI and database should still fail
        config.insert(RString::from("database"), RString::from("testdb"));
        assert!(sink.validate_config(config.clone()).is_err());

        // With all required configs should succeed
        config.insert(RString::from("collection"), RString::from("testcol"));
        assert!(sink.validate_config(config).is_ok());
    }

    #[test]
    fn test_json_to_bson() {
        use serde_json::json;

        // Test null
        let val = json!(null);
        assert!(matches!(json_to_bson(&val), Some(mongodb::bson::Bson::Null)));

        // Test boolean
        let val = json!(true);
        assert!(matches!(json_to_bson(&val), Some(mongodb::bson::Bson::Boolean(true))));

        // Test number
        let val = json!(42);
        assert!(matches!(json_to_bson(&val), Some(mongodb::bson::Bson::Int64(42))));

        // Test string
        let val = json!("hello");
        if let Some(mongodb::bson::Bson::String(s)) = json_to_bson(&val) {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected string");
        }
    }
}
