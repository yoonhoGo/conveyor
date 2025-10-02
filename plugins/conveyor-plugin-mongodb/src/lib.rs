//! MongoDB Plugin for Conveyor - Version 2 (Unified Stage API)
//!
//! Provides MongoDB source and sink functionality as a unified stage.
//! Supports database reads and writes with query filtering.

use conveyor_plugin_api::{
    FfiDataFormat, PluginCapability, PluginDeclaration, StageType, PLUGIN_API_VERSION,
    RBox, RBoxError, RHashMap, RResult, RString, RVec, rstr, ROk, RErr,
};
use conveyor_plugin_api::traits::{FfiExecutionContext, FfiStage, FfiStage_TO};
use conveyor_plugin_api::sabi_trait::prelude::*;
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, FindOptions},
    Client,
};
use serde_json::Value;
use std::collections::HashMap;

/// MongoDB Stage - unified source and sink
pub struct MongoDbStage {
    name: String,
    stage_type: StageType,
}

impl MongoDbStage {
    fn new(name: String, stage_type: StageType) -> Self {
        Self { name, stage_type }
    }

    /// Execute as MongoDB source (read data from MongoDB)
    async fn execute_source_async(&self, config: &HashMap<String, String>) -> RResult<FfiDataFormat, RBoxError> {
        // Get MongoDB URI
        let uri = match config.get("uri") {
            Some(u) => u,
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing required 'uri' configuration"))),
        };

        // Get database name
        let database = match config.get("database") {
            Some(d) => d,
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing required 'database' configuration"))),
        };

        // Get collection name
        let collection_name = match config.get("collection") {
            Some(c) => c,
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing required 'collection' configuration"))),
        };

        // Parse limit (default: no limit)
        let limit: Option<i64> = config
            .get("limit")
            .and_then(|l| l.parse().ok());

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
        let filter = if let Some(query_str) = config.get("query") {
            match serde_json::from_str::<Value>(query_str) {
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

    /// Execute as MongoDB sink (write data to MongoDB)
    async fn execute_sink_async(&self, input_data: &FfiDataFormat, config: &HashMap<String, String>) -> RResult<FfiDataFormat, RBoxError> {
        // Get MongoDB URI
        let uri = match config.get("uri") {
            Some(u) => u,
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing required 'uri' configuration"))),
        };

        // Get database name
        let database = match config.get("database") {
            Some(d) => d,
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing required 'database' configuration"))),
        };

        // Get collection name
        let collection_name = match config.get("collection") {
            Some(c) => c,
            None => return RErr(RBoxError::from_fmt(&format_args!("Missing required 'collection' configuration"))),
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
        let records = match input_data.to_json_records() {
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
            return ROk(input_data.clone());
        }

        // Insert documents
        match collection.insert_many(documents).await {
            Ok(_) => ROk(input_data.clone()),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!("MongoDB insert failed: {}", e))),
        }
    }
}

impl FfiStage for MongoDbStage {
    fn name(&self) -> conveyor_plugin_api::RStr<'_> {
        self.name.as_str().into()
    }

    fn stage_type(&self) -> StageType {
        self.stage_type
    }

    fn execute(&self, context: FfiExecutionContext) -> RResult<FfiDataFormat, RBoxError> {
        // Convert config to HashMap
        let config: HashMap<String, String> = context.config.into_iter()
            .map(|tuple| (tuple.0.to_string(), tuple.1.to_string()))
            .collect();

        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return RErr(RBoxError::from_fmt(&format_args!("Failed to create runtime: {}", e))),
        };

        runtime.block_on(async {
            match self.stage_type {
                StageType::Source => {
                    // Source mode: read from MongoDB
                    self.execute_source_async(&config).await
                }
                StageType::Sink => {
                    // Sink mode: write to MongoDB
                    // Get input data (should have exactly one input)
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => return RErr(RBoxError::from_fmt(&format_args!("MongoDB sink requires input data"))),
                    };

                    self.execute_sink_async(&input_data, &config).await
                }
                StageType::Transform => {
                    RErr(RBoxError::from_fmt(&format_args!("MongoDB transform not supported in this plugin")))
                }
            }
        })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Check required fields
        if !config.contains_key("uri") {
            return RErr(RBoxError::from_fmt(&format_args!("Missing required 'uri' configuration")));
        }

        if !config.contains_key("database") {
            return RErr(RBoxError::from_fmt(&format_args!("Missing required 'database' configuration")));
        }

        if !config.contains_key("collection") {
            return RErr(RBoxError::from_fmt(&format_args!("Missing required 'collection' configuration")));
        }

        ROk(())
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

// Factory functions
#[no_mangle]
pub extern "C" fn create_mongodb_source() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(MongoDbStage::new("mongodb".to_string(), StageType::Source), TD_Opaque)
}

#[no_mangle]
pub extern "C" fn create_mongodb_sink() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(MongoDbStage::new("mongodb".to_string(), StageType::Sink), TD_Opaque)
}

// Plugin capabilities
extern "C" fn get_capabilities() -> RVec<PluginCapability> {
    vec![
        PluginCapability {
            name: RString::from("mongodb"),
            stage_type: StageType::Source,
            description: RString::from("MongoDB source - read data from MongoDB collections"),
            factory_symbol: RString::from("create_mongodb_source"),
        },
        PluginCapability {
            name: RString::from("mongodb"),
            stage_type: StageType::Sink,
            description: RString::from("MongoDB sink - write data to MongoDB collections"),
            factory_symbol: RString::from("create_mongodb_sink"),
        },
    ].into()
}

// Plugin declaration
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("mongodb"),
    version: rstr!("1.0.0"),
    description: rstr!("MongoDB source and sink plugin for database integration"),
    get_capabilities,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mongodb_source() {
        let stage = MongoDbStage::new("mongodb".to_string(), StageType::Source);
        assert_eq!(stage.name(), "mongodb");
        assert_eq!(stage.stage_type(), StageType::Source);
    }

    #[test]
    fn test_mongodb_sink() {
        let stage = MongoDbStage::new("mongodb".to_string(), StageType::Sink);
        assert_eq!(stage.name(), "mongodb");
        assert_eq!(stage.stage_type(), StageType::Sink);
    }

    #[test]
    fn test_plugin_declaration() {
        assert_eq!(_plugin_declaration.name, "mongodb");
        assert_eq!(_plugin_declaration.version, "1.0.0");
        assert!(_plugin_declaration.is_compatible());
    }

    #[test]
    fn test_source_validation() {
        let stage = MongoDbStage::new("mongodb".to_string(), StageType::Source);
        let mut config = RHashMap::new();

        // Missing all configs should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // With only URI should still fail
        config.insert(RString::from("uri"), RString::from("mongodb://localhost:27017"));
        assert!(stage.validate_config(config.clone()).is_err());

        // With URI and database should still fail
        config.insert(RString::from("database"), RString::from("testdb"));
        assert!(stage.validate_config(config.clone()).is_err());

        // With all required configs should succeed
        config.insert(RString::from("collection"), RString::from("testcol"));
        assert!(stage.validate_config(config).is_ok());
    }

    #[test]
    fn test_sink_validation() {
        let stage = MongoDbStage::new("mongodb".to_string(), StageType::Sink);
        let mut config = RHashMap::new();

        // Missing all configs should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // With only URI should still fail
        config.insert(RString::from("uri"), RString::from("mongodb://localhost:27017"));
        assert!(stage.validate_config(config.clone()).is_err());

        // With URI and database should still fail
        config.insert(RString::from("database"), RString::from("testdb"));
        assert!(stage.validate_config(config.clone()).is_err());

        // With all required configs should succeed
        config.insert(RString::from("collection"), RString::from("testcol"));
        assert!(stage.validate_config(config).is_ok());
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

    #[test]
    fn test_capabilities() {
        let caps = get_capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].name.as_str(), "mongodb");
        assert_eq!(caps[0].stage_type, StageType::Source);
        assert_eq!(caps[1].stage_type, StageType::Sink);
    }
}
