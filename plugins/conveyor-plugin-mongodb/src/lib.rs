//! MongoDB Plugin for Conveyor - Operation-based API
//!
//! Provides MongoDB operations: find, findOne, createOne, createMany,
//! updateOne, updateMany, deleteOne, deleteMany, replaceOne, replaceMany

use conveyor_plugin_api::sabi_trait::prelude::*;
use conveyor_plugin_api::traits::{FfiExecutionContext, FfiStage, FfiStage_TO};
use conveyor_plugin_api::{
    rstr, FfiConfigParameter, FfiDataFormat, FfiParameterType, FfiStageMetadata, PluginCapability,
    PluginDeclaration, RBox, RBoxError, RErr, RHashMap, ROk, RResult, RString, RVec, StageType,
    PLUGIN_API_VERSION,
};
use mongodb::{
    bson::Document,
    options::{ClientOptions, FindOneOptions, FindOptions, ReplaceOptions, UpdateOptions},
    Client,
};
use serde_json::Value;
use std::collections::HashMap;

/// MongoDB operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MongoOperation {
    Find,
    FindOne,
    InsertOne,
    InsertMany,
    UpdateOne,
    UpdateMany,
    DeleteOne,
    DeleteMany,
    ReplaceOne,
    ReplaceMany,
}

/// MongoDB Stage - operation-based
pub struct MongoDbStage {
    name: String,
    operation: MongoOperation,
    stage_type: StageType,
}

impl MongoDbStage {
    fn new(name: String, operation: MongoOperation, stage_type: StageType) -> Self {
        Self {
            name,
            operation,
            stage_type,
        }
    }

    /// Execute find operation - read multiple documents
    async fn execute_find_async(
        &self,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build query filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Build find options
        let mut find_options = FindOptions::default();
        if let Some(limit_str) = config.get("limit") {
            if let Ok(limit) = limit_str.parse::<i64>() {
                find_options.limit = Some(limit);
            }
        }

        // Execute query
        let mut cursor = match collection.find(filter).with_options(find_options).await {
            Ok(c) => c,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "MongoDB find failed: {}",
                    e
                )))
            }
        };

        // Collect results
        let mut records: Vec<HashMap<String, Value>> = Vec::new();
        use futures::stream::TryStreamExt;
        loop {
            match cursor.try_next().await {
                Ok(Some(doc)) => {
                    if let Ok(json_str) = serde_json::to_string(&doc) {
                        if let Ok(json_val) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(obj) = json_val.as_object() {
                                let record: HashMap<String, Value> =
                                    obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                records.push(record);
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Failed to fetch document: {}",
                        e
                    )))
                }
            }
        }

        FfiDataFormat::from_json_records(&records)
    }

    /// Execute findOne operation - read single document
    async fn execute_find_one_async(
        &self,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build query filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Execute query
        let doc = match collection
            .find_one(filter)
            .with_options(FindOneOptions::default())
            .await
        {
            Ok(Some(d)) => d,
            Ok(None) => {
                // No document found, return empty result
                let empty: Vec<HashMap<String, Value>> = Vec::new();
                return FfiDataFormat::from_json_records(&empty);
            }
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "MongoDB findOne failed: {}",
                    e
                )))
            }
        };

        // Convert to JSON record
        if let Ok(json_str) = serde_json::to_string(&doc) {
            if let Ok(json_val) = serde_json::from_str::<Value>(&json_str) {
                if let Some(obj) = json_val.as_object() {
                    let record: HashMap<String, Value> =
                        obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    return FfiDataFormat::from_json_records(&[record]);
                }
            }
        }

        RErr(RBoxError::from_fmt(&format_args!(
            "Failed to convert document to JSON"
        )))
    }

    /// Execute insertOne operation - insert single document
    async fn execute_insert_one_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Convert input data to document
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        if records.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "No data provided for insertOne"
            )));
        }

        let record = &records[0];
        let doc = match self.json_record_to_bson(record) {
            ROk(d) => d,
            RErr(e) => return RErr(e),
        };

        // Insert document
        match collection.insert_one(doc).await {
            Ok(_) => ROk(input_data.clone()),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "MongoDB insertOne failed: {}",
                e
            ))),
        }
    }

    /// Execute insertMany operation - insert multiple documents
    async fn execute_insert_many_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Convert input data to documents
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        if records.is_empty() {
            return ROk(input_data.clone());
        }

        let documents: Vec<Document> = records
            .iter()
            .filter_map(|record| match self.json_record_to_bson(record) {
                ROk(d) => Some(d),
                RErr(_) => None,
            })
            .collect();

        if documents.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "No valid documents to insert"
            )));
        }

        // Insert documents
        match collection.insert_many(documents).await {
            Ok(_) => ROk(input_data.clone()),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "MongoDB insertMany failed: {}",
                e
            ))),
        }
    }

    /// Execute updateOne operation - update single document
    async fn execute_update_one_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Build update document from input data
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        if records.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "No data provided for updateOne"
            )));
        }

        let update_doc = match self.json_record_to_bson(&records[0]) {
            ROk(d) => d,
            RErr(e) => return RErr(e),
        };

        // Wrap in $set operator
        let update = mongodb::bson::doc! { "$set": update_doc };

        // Execute update
        match collection
            .update_one(filter, update)
            .with_options(UpdateOptions::default())
            .await
        {
            Ok(_) => ROk(input_data.clone()),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "MongoDB updateOne failed: {}",
                e
            ))),
        }
    }

    /// Execute updateMany operation - update multiple documents
    async fn execute_update_many_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Build update document from input data
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        if records.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "No data provided for updateMany"
            )));
        }

        let update_doc = match self.json_record_to_bson(&records[0]) {
            ROk(d) => d,
            RErr(e) => return RErr(e),
        };

        // Wrap in $set operator
        let update = mongodb::bson::doc! { "$set": update_doc };

        // Execute update
        match collection
            .update_many(filter, update)
            .with_options(UpdateOptions::default())
            .await
        {
            Ok(_) => ROk(input_data.clone()),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "MongoDB updateMany failed: {}",
                e
            ))),
        }
    }

    /// Execute deleteOne operation - delete single document
    async fn execute_delete_one_async(
        &self,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Execute delete
        match collection.delete_one(filter).await {
            Ok(_) => {
                let empty: Vec<HashMap<String, Value>> = Vec::new();
                FfiDataFormat::from_json_records(&empty)
            }
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "MongoDB deleteOne failed: {}",
                e
            ))),
        }
    }

    /// Execute deleteMany operation - delete multiple documents
    async fn execute_delete_many_async(
        &self,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Execute delete
        match collection.delete_many(filter).await {
            Ok(_) => {
                let empty: Vec<HashMap<String, Value>> = Vec::new();
                FfiDataFormat::from_json_records(&empty)
            }
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "MongoDB deleteMany failed: {}",
                e
            ))),
        }
    }

    /// Execute replaceOne operation - replace single document
    async fn execute_replace_one_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Build replacement document from input data
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        if records.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "No data provided for replaceOne"
            )));
        }

        let replacement = match self.json_record_to_bson(&records[0]) {
            ROk(d) => d,
            RErr(e) => return RErr(e),
        };

        // Execute replace
        match collection
            .replace_one(filter, replacement)
            .with_options(ReplaceOptions::default())
            .await
        {
            Ok(_) => ROk(input_data.clone()),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "MongoDB replaceOne failed: {}",
                e
            ))),
        }
    }

    /// Execute replaceMany operation - replace multiple documents
    /// Note: MongoDB doesn't have a native replaceMany, so we iterate
    async fn execute_replace_many_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Get all documents matching filter
        let mut cursor = match collection.find(filter.clone()).await {
            Ok(c) => c,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "MongoDB find failed: {}",
                    e
                )))
            }
        };

        // Build replacement document from input data
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        if records.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "No data provided for replaceMany"
            )));
        }

        let replacement = match self.json_record_to_bson(&records[0]) {
            ROk(d) => d,
            RErr(e) => return RErr(e),
        };

        // Replace each document
        use futures::stream::TryStreamExt;
        loop {
            match cursor.try_next().await {
                Ok(Some(doc)) => {
                    if let Some(id) = doc.get("_id") {
                        let id_filter = mongodb::bson::doc! { "_id": id };
                        if let Err(e) = collection.replace_one(id_filter, replacement.clone()).await
                        {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "MongoDB replaceMany failed: {}",
                                e
                            )));
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Failed to fetch document: {}",
                        e
                    )))
                }
            }
        }

        ROk(input_data.clone())
    }

    // Helper methods

    /// Connect to MongoDB and return client, database name, and collection name
    async fn connect_mongodb(
        &self,
        config: &HashMap<String, String>,
    ) -> RResult<(Client, String, String), RBoxError> {
        let uri = match config.get("uri") {
            Some(u) => u,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'uri' configuration"
                )))
            }
        };

        let database = match config.get("database") {
            Some(d) => d,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'database' configuration"
                )))
            }
        };

        let collection = match config.get("collection") {
            Some(c) => c,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'collection' configuration"
                )))
            }
        };

        let client_options = match ClientOptions::parse(uri).await {
            Ok(opts) => opts,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to parse MongoDB URI: {}",
                    e
                )))
            }
        };

        let client = match Client::with_options(client_options) {
            Ok(c) => c,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create MongoDB client: {}",
                    e
                )))
            }
        };

        ROk((client, database.clone(), collection.clone()))
    }

    /// Parse query from config
    fn parse_query(&self, config: &HashMap<String, String>) -> RResult<Document, RBoxError> {
        if let Some(query_str) = config.get("query") {
            match serde_json::from_str::<Value>(query_str) {
                Ok(Value::Object(obj)) => {
                    let mut filter_doc = Document::new();
                    for (key, value) in obj {
                        if let Some(bson_val) = json_to_bson(&value) {
                            filter_doc.insert(key, bson_val);
                        }
                    }
                    ROk(filter_doc)
                }
                _ => ROk(Document::new()),
            }
        } else {
            ROk(Document::new())
        }
    }

    /// Convert JSON record to BSON document
    fn json_record_to_bson(&self, record: &HashMap<String, Value>) -> RResult<Document, RBoxError> {
        let mut doc = Document::new();
        for (key, value) in record {
            if let Some(bson_val) = json_to_bson(value) {
                doc.insert(key.clone(), bson_val);
            }
        }
        if doc.is_empty() {
            RErr(RBoxError::from_fmt(&format_args!(
                "Failed to convert JSON to BSON"
            )))
        } else {
            ROk(doc)
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
        let config: HashMap<String, String> = context
            .config
            .into_iter()
            .map(|tuple| (tuple.0.to_string(), tuple.1.to_string()))
            .collect();

        // Use tokio runtime to execute async code
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Failed to create runtime: {}",
                    e
                )))
            }
        };

        runtime.block_on(async {
            match self.operation {
                MongoOperation::Find => self.execute_find_async(&config).await,
                MongoOperation::FindOne => self.execute_find_one_async(&config).await,
                MongoOperation::InsertOne => {
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "insertOne requires input data"
                            )))
                        }
                    };
                    self.execute_insert_one_async(&input_data, &config).await
                }
                MongoOperation::InsertMany => {
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "insertMany requires input data"
                            )))
                        }
                    };
                    self.execute_insert_many_async(&input_data, &config).await
                }
                MongoOperation::UpdateOne => {
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "updateOne requires input data"
                            )))
                        }
                    };
                    self.execute_update_one_async(&input_data, &config).await
                }
                MongoOperation::UpdateMany => {
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "updateMany requires input data"
                            )))
                        }
                    };
                    self.execute_update_many_async(&input_data, &config).await
                }
                MongoOperation::DeleteOne => self.execute_delete_one_async(&config).await,
                MongoOperation::DeleteMany => self.execute_delete_many_async(&config).await,
                MongoOperation::ReplaceOne => {
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "replaceOne requires input data"
                            )))
                        }
                    };
                    self.execute_replace_one_async(&input_data, &config).await
                }
                MongoOperation::ReplaceMany => {
                    let input_data = match context.inputs.into_iter().next() {
                        Some(tuple) => tuple.1,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "replaceMany requires input data"
                            )))
                        }
                    };
                    self.execute_replace_many_async(&input_data, &config).await
                }
            }
        })
    }

    fn validate_config(&self, config: RHashMap<RString, RString>) -> RResult<(), RBoxError> {
        // Check required fields
        if !config.contains_key("uri") {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Missing required 'uri' configuration"
            )));
        }

        if !config.contains_key("database") {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Missing required 'database' configuration"
            )));
        }

        if !config.contains_key("collection") {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Missing required 'collection' configuration"
            )));
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
            } else {
                n.as_f64().map(mongodb::bson::Bson::Double)
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

// Factory functions for each operation
#[no_mangle]
pub extern "C" fn create_mongodb_find() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-find".to_string(),
            MongoOperation::Find,
            StageType::Source,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_findone() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-findone".to_string(),
            MongoOperation::FindOne,
            StageType::Source,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_insertone() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-insertone".to_string(),
            MongoOperation::InsertOne,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_insertmany() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-insertmany".to_string(),
            MongoOperation::InsertMany,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_updateone() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-updateone".to_string(),
            MongoOperation::UpdateOne,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_updatemany() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-updatemany".to_string(),
            MongoOperation::UpdateMany,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_deleteone() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-deleteone".to_string(),
            MongoOperation::DeleteOne,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_deletemany() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-deletemany".to_string(),
            MongoOperation::DeleteMany,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_replaceone() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-replaceone".to_string(),
            MongoOperation::ReplaceOne,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_replacemany() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-replacemany".to_string(),
            MongoOperation::ReplaceMany,
            StageType::Sink,
        ),
        TD_Opaque,
    )
}

// ============================================================================
// Metadata Helper Functions
// ============================================================================

/// Create common MongoDB parameters (uri, database, collection)
fn common_mongodb_parameters() -> Vec<FfiConfigParameter> {
    vec![
        FfiConfigParameter::required(
            "uri",
            FfiParameterType::String,
            "MongoDB connection string (e.g., mongodb://localhost:27017)",
        ),
        FfiConfigParameter::required("database", FfiParameterType::String, "Database name"),
        FfiConfigParameter::required("collection", FfiParameterType::String, "Collection name"),
    ]
}

/// Create metadata for find operation
fn create_find_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.extend(vec![
        FfiConfigParameter::optional(
            "query",
            FfiParameterType::String,
            "{}",
            "MongoDB query filter as JSON string (e.g., '{\"status\": \"active\"}')",
        ),
        FfiConfigParameter::optional(
            "limit",
            FfiParameterType::Integer,
            "",
            "Maximum number of documents to return",
        ),
    ]);

    FfiStageMetadata::new(
        "mongodb.find",
        "Find multiple documents from MongoDB collection",
        "Executes a MongoDB find query and returns all matching documents. \
         Supports filtering with MongoDB query syntax and limiting results. \
         Uses cursor-based iteration for efficient large result sets.",
        params,
        vec!["mongodb", "database", "source", "find", "query"],
    )
}

/// Create metadata for findOne operation
fn create_findone_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::optional(
        "query",
        FfiParameterType::String,
        "{}",
        "MongoDB query filter as JSON string",
    ));

    FfiStageMetadata::new(
        "mongodb.findOne",
        "Find single document from MongoDB collection",
        "Executes a MongoDB findOne query and returns the first matching document. \
         Returns empty result if no document matches the query.",
        params,
        vec!["mongodb", "database", "source", "findOne", "query"],
    )
}

/// Create metadata for insertOne operation
fn create_insertone_metadata() -> FfiStageMetadata {
    FfiStageMetadata::new(
        "mongodb.insertOne",
        "Insert single document into MongoDB collection",
        "Inserts a single document from the input data into MongoDB. \
         Takes the first record from the input dataset.",
        common_mongodb_parameters(),
        vec!["mongodb", "database", "sink", "insert"],
    )
}

/// Create metadata for insertMany operation
fn create_insertmany_metadata() -> FfiStageMetadata {
    FfiStageMetadata::new(
        "mongodb.insertMany",
        "Insert multiple documents into MongoDB collection",
        "Inserts all documents from the input data into MongoDB in a single batch operation. \
         Efficient for bulk inserts.",
        common_mongodb_parameters(),
        vec!["mongodb", "database", "sink", "insert", "bulk"],
    )
}

/// Create metadata for updateOne operation
fn create_updateone_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::required(
        "query",
        FfiParameterType::String,
        "MongoDB query filter to match the document to update",
    ));

    FfiStageMetadata::new(
        "mongodb.updateOne",
        "Update single document in MongoDB collection",
        "Updates the first document matching the query filter using $set operator. \
         Update fields are taken from the first record in input data.",
        params,
        vec!["mongodb", "database", "sink", "update"],
    )
}

/// Create metadata for updateMany operation
fn create_updatemany_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::required(
        "query",
        FfiParameterType::String,
        "MongoDB query filter to match documents to update",
    ));

    FfiStageMetadata::new(
        "mongodb.updateMany",
        "Update multiple documents in MongoDB collection",
        "Updates all documents matching the query filter using $set operator. \
         Update fields are taken from the first record in input data.",
        params,
        vec!["mongodb", "database", "sink", "update", "bulk"],
    )
}

/// Create metadata for deleteOne operation
fn create_deleteone_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::required(
        "query",
        FfiParameterType::String,
        "MongoDB query filter to match the document to delete",
    ));

    FfiStageMetadata::new(
        "mongodb.deleteOne",
        "Delete single document from MongoDB collection",
        "Deletes the first document matching the query filter.",
        params,
        vec!["mongodb", "database", "sink", "delete"],
    )
}

/// Create metadata for deleteMany operation
fn create_deletemany_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::required(
        "query",
        FfiParameterType::String,
        "MongoDB query filter to match documents to delete",
    ));

    FfiStageMetadata::new(
        "mongodb.deleteMany",
        "Delete multiple documents from MongoDB collection",
        "Deletes all documents matching the query filter.",
        params,
        vec!["mongodb", "database", "sink", "delete", "bulk"],
    )
}

/// Create metadata for replaceOne operation
fn create_replaceone_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::required(
        "query",
        FfiParameterType::String,
        "MongoDB query filter to match the document to replace",
    ));

    FfiStageMetadata::new(
        "mongodb.replaceOne",
        "Replace single document in MongoDB collection",
        "Replaces the entire document matching the query filter with new document from input data. \
         Unlike update, this replaces the whole document (except _id).",
        params,
        vec!["mongodb", "database", "sink", "replace"],
    )
}

/// Create metadata for replaceMany operation
fn create_replacemany_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::required(
        "query",
        FfiParameterType::String,
        "MongoDB query filter to match documents to replace",
    ));

    FfiStageMetadata::new(
        "mongodb.replaceMany",
        "Replace multiple documents in MongoDB collection",
        "Replaces all documents matching the query filter with new document from input data. \
         Iterates through matching documents and replaces each one individually.",
        params,
        vec!["mongodb", "database", "sink", "replace", "bulk"],
    )
}

// ============================================================================
// Plugin Capabilities
// ============================================================================

// Plugin capabilities
extern "C" fn get_capabilities() -> RVec<PluginCapability> {
    vec![
        PluginCapability::new(
            "mongodb.find",
            StageType::Source,
            "MongoDB find - read multiple documents",
            "create_mongodb_find",
            create_find_metadata(),
        ),
        PluginCapability::new(
            "mongodb.findOne",
            StageType::Source,
            "MongoDB findOne - read single document",
            "create_mongodb_findone",
            create_findone_metadata(),
        ),
        PluginCapability::new(
            "mongodb.insertOne",
            StageType::Sink,
            "MongoDB insertOne - insert single document",
            "create_mongodb_insertone",
            create_insertone_metadata(),
        ),
        PluginCapability::new(
            "mongodb.insertMany",
            StageType::Sink,
            "MongoDB insertMany - insert multiple documents",
            "create_mongodb_insertmany",
            create_insertmany_metadata(),
        ),
        PluginCapability::new(
            "mongodb.updateOne",
            StageType::Sink,
            "MongoDB updateOne - update single document",
            "create_mongodb_updateone",
            create_updateone_metadata(),
        ),
        PluginCapability::new(
            "mongodb.updateMany",
            StageType::Sink,
            "MongoDB updateMany - update multiple documents",
            "create_mongodb_updatemany",
            create_updatemany_metadata(),
        ),
        PluginCapability::new(
            "mongodb.deleteOne",
            StageType::Sink,
            "MongoDB deleteOne - delete single document",
            "create_mongodb_deleteone",
            create_deleteone_metadata(),
        ),
        PluginCapability::new(
            "mongodb.deleteMany",
            StageType::Sink,
            "MongoDB deleteMany - delete multiple documents",
            "create_mongodb_deletemany",
            create_deletemany_metadata(),
        ),
        PluginCapability::new(
            "mongodb.replaceOne",
            StageType::Sink,
            "MongoDB replaceOne - replace single document",
            "create_mongodb_replaceone",
            create_replaceone_metadata(),
        ),
        PluginCapability::new(
            "mongodb.replaceMany",
            StageType::Sink,
            "MongoDB replaceMany - replace multiple documents",
            "create_mongodb_replacemany",
            create_replacemany_metadata(),
        ),
    ]
    .into()
}

// Plugin declaration
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("mongodb"),
    version: rstr!("2.0.0"),
    description: rstr!(
        "MongoDB plugin with operation-based API (find, findOne, insert, update, delete, replace)"
    ),
    get_capabilities,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_declaration() {
        assert_eq!(_plugin_declaration.name, "mongodb");
        assert_eq!(_plugin_declaration.version, "2.0.0");
        assert!(_plugin_declaration.is_compatible());
    }

    #[test]
    fn test_capabilities() {
        let caps = get_capabilities();
        assert_eq!(caps.len(), 10);

        // Check find operation
        assert_eq!(caps[0].name.as_str(), "mongodb.find");
        assert_eq!(caps[0].stage_type, StageType::Source);

        // Check findOne operation
        assert_eq!(caps[1].name.as_str(), "mongodb.findOne");
        assert_eq!(caps[1].stage_type, StageType::Source);

        // Check insertOne operation
        assert_eq!(caps[2].name.as_str(), "mongodb.insertOne");
        assert_eq!(caps[2].stage_type, StageType::Sink);

        // Check insertMany operation
        assert_eq!(caps[3].name.as_str(), "mongodb.insertMany");
        assert_eq!(caps[3].stage_type, StageType::Sink);
    }

    #[test]
    fn test_validation() {
        let stage = MongoDbStage::new(
            "mongodb.find".to_string(),
            MongoOperation::Find,
            StageType::Source,
        );
        let mut config = RHashMap::new();

        // Missing all configs should fail
        assert!(stage.validate_config(config.clone()).is_err());

        // With only URI should still fail
        config.insert(
            RString::from("uri"),
            RString::from("mongodb://localhost:27017"),
        );
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
        assert!(matches!(
            json_to_bson(&val),
            Some(mongodb::bson::Bson::Null)
        ));

        // Test boolean
        let val = json!(true);
        assert!(matches!(
            json_to_bson(&val),
            Some(mongodb::bson::Bson::Boolean(true))
        ));

        // Test number
        let val = json!(42);
        assert!(matches!(
            json_to_bson(&val),
            Some(mongodb::bson::Bson::Int64(42))
        ));

        // Test string
        let val = json!("hello");
        if let Some(mongodb::bson::Bson::String(s)) = json_to_bson(&val) {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected string");
        }
    }
}
