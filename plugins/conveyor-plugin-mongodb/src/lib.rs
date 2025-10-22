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
use handlebars::Handlebars;

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
    Aggregate,
    BulkWrite,
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

    /// Render template with input data context
    fn render_template(
        &self,
        template: &str,
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<String, RBoxError> {
        let handlebars = Handlebars::new();

        // Create context from input data
        let context = if let Some(data) = input_data {
            match data.to_json_records() {
                ROk(records) if !records.is_empty() => {
                    // Use first record as context
                    serde_json::to_value(&records[0]).unwrap_or(serde_json::json!({}))
                }
                _ => serde_json::json!({}),
            }
        } else {
            serde_json::json!({})
        };

        match handlebars.render_template(template, &context) {
            Ok(rendered) => ROk(rendered),
            Err(e) => RErr(RBoxError::from_fmt(&format_args!(
                "Template rendering failed: {}",
                e
            ))),
        }
    }

    /// Render config value with template support
    fn render_config_value(
        &self,
        config: &HashMap<String, String>,
        key: &str,
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<Option<String>, RBoxError> {
        if let Some(value) = config.get(key) {
            match self.render_template(value, input_data) {
                ROk(rendered) => ROk(Some(rendered)),
                RErr(e) => RErr(e),
            }
        } else {
            ROk(None)
        }
    }

    /// Execute find operation - read multiple documents
    async fn execute_find_async(
        &self,
        config: &HashMap<String, String>,
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config, input_data).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build query filter
        let filter = match self.parse_query(config, input_data) {
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
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config, input_data).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build query filter
        let filter = match self.parse_query(config, input_data) {
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
        let (client, db_name, collection_name) = match self.connect_mongodb(config, Some(input_data)).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Try to get document from input data first, then fall back to config
        let doc = match input_data.to_json_records() {
            ROk(records) if !records.is_empty() => {
                // Use first record from input data
                match self.json_record_to_bson(&records[0]) {
                    ROk(d) => d,
                    RErr(e) => return RErr(e),
                }
            }
            _ => {
                // Input data is empty or invalid, try config
                match self.parse_document_from_config(config) {
                    ROk(Some(d)) => d,
                    ROk(None) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "No data provided for insertOne. Provide either input data or 'document' in config"
                        )))
                    }
                    RErr(e) => return RErr(e),
                }
            }
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
        let (client, db_name, collection_name) = match self.connect_mongodb(config, Some(input_data)).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Try to get documents from input data first, then fall back to config
        let documents: Vec<Document> = match input_data.to_json_records() {
            ROk(records) if !records.is_empty() => {
                // Use records from input data
                records
                    .iter()
                    .filter_map(|record| match self.json_record_to_bson(record) {
                        ROk(d) => Some(d),
                        RErr(_) => None,
                    })
                    .collect()
            }
            _ => {
                // Input data is empty or invalid, try config
                match self.parse_documents_from_config(config) {
                    ROk(Some(docs)) => docs,
                    ROk(None) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "No data provided for insertMany. Provide either input data or 'documents' in config"
                        )))
                    }
                    RErr(e) => return RErr(e),
                }
            }
        };

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
        let (client, db_name, collection_name) = match self.connect_mongodb(config, Some(input_data)).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config, Some(input_data)) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Try to get update document from input data first, then fall back to config
        let update_doc = match input_data.to_json_records() {
            ROk(records) if !records.is_empty() => {
                // Use first record from input data
                match self.json_record_to_bson(&records[0]) {
                    ROk(d) => d,
                    RErr(e) => return RErr(e),
                }
            }
            _ => {
                // Input data is empty or invalid, try config
                match self.parse_update_from_config(config) {
                    ROk(Some(d)) => d,
                    ROk(None) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "No data provided for updateOne. Provide either input data or 'update' in config"
                        )))
                    }
                    RErr(e) => return RErr(e),
                }
            }
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
        let (client, db_name, collection_name) = match self.connect_mongodb(config, Some(input_data)).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config, Some(input_data)) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Try to get update document from input data first, then fall back to config
        let update_doc = match input_data.to_json_records() {
            ROk(records) if !records.is_empty() => {
                // Use first record from input data
                match self.json_record_to_bson(&records[0]) {
                    ROk(d) => d,
                    RErr(e) => return RErr(e),
                }
            }
            _ => {
                // Input data is empty or invalid, try config
                match self.parse_update_from_config(config) {
                    ROk(Some(d)) => d,
                    ROk(None) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "No data provided for updateMany. Provide either input data or 'update' in config"
                        )))
                    }
                    RErr(e) => return RErr(e),
                }
            }
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
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config, input_data).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config, input_data) {
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
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config, input_data).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config, input_data) {
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
        let (client, db_name, collection_name) = match self.connect_mongodb(config, Some(input_data)).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config, Some(input_data)) {
            ROk(f) => f,
            RErr(e) => return RErr(e),
        };

        // Try to get replacement document from input data first, then fall back to config
        let replacement = match input_data.to_json_records() {
            ROk(records) if !records.is_empty() => {
                // Use first record from input data
                match self.json_record_to_bson(&records[0]) {
                    ROk(d) => d,
                    RErr(e) => return RErr(e),
                }
            }
            _ => {
                // Input data is empty or invalid, try config
                match self.parse_replacement_from_config(config) {
                    ROk(Some(d)) => d,
                    ROk(None) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "No data provided for replaceOne. Provide either input data or 'replacement' in config"
                        )))
                    }
                    RErr(e) => return RErr(e),
                }
            }
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
        let (client, db_name, collection_name) = match self.connect_mongodb(config, Some(input_data)).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Build filter
        let filter = match self.parse_query(config, Some(input_data)) {
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

        // Try to get replacement document from input data first, then fall back to config
        let replacement = match input_data.to_json_records() {
            ROk(records) if !records.is_empty() => {
                // Use first record from input data
                match self.json_record_to_bson(&records[0]) {
                    ROk(d) => d,
                    RErr(e) => return RErr(e),
                }
            }
            _ => {
                // Input data is empty or invalid, try config
                match self.parse_replacement_from_config(config) {
                    ROk(Some(d)) => d,
                    ROk(None) => {
                        return RErr(RBoxError::from_fmt(&format_args!(
                            "No data provided for replaceMany. Provide either input data or 'replacement' in config"
                        )))
                    }
                    RErr(e) => return RErr(e),
                }
            }
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

    /// Execute bulkWrite operation - batch write operations
    async fn execute_bulk_write_async(
        &self,
        input_data: &FfiDataFormat,
        config: &HashMap<String, String>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config, Some(input_data)).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Convert input data to operations
        let records = match input_data.to_json_records() {
            ROk(r) => r,
            RErr(e) => return RErr(e),
        };

        if records.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "No operations provided for bulkWrite"
            )));
        }

        // Parse operations from input data
        // Expected format: { "operations": [...] }
        let operations_value = match records[0].get("operations") {
            Some(v) => v,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing 'operations' field in input data"
                )))
            }
        };

        let operations_array = match operations_value.as_array() {
            Some(arr) => arr,
            None => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "'operations' must be an array"
                )))
            }
        };

        // Execute each operation sequentially
        let mut insert_count = 0;
        let mut update_count = 0;
        let mut delete_count = 0;
        let mut replace_count = 0;

        for operation in operations_array {
            let op_obj = match operation.as_object() {
                Some(obj) => obj,
                None => continue,
            };

            let op_type = match op_obj.get("type").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => continue,
            };

            match op_type {
                "insertOne" => {
                    if let Some(Value::Object(doc_obj)) = op_obj.get("document") {
                        let doc_map: HashMap<String, Value> = doc_obj
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let doc = match self.json_record_to_bson(&doc_map) {
                            ROk(d) => d,
                            RErr(e) => return RErr(e),
                        };
                        match collection.insert_one(doc).await {
                            Ok(_) => insert_count += 1,
                            Err(e) => {
                                return RErr(RBoxError::from_fmt(&format_args!(
                                    "bulkWrite insertOne failed: {}",
                                    e
                                )))
                            }
                        }
                    }
                }
                "insertMany" => {
                    if let Some(Value::Array(docs_array)) = op_obj.get("documents") {
                        let documents: Vec<Document> = docs_array
                            .iter()
                            .filter_map(|val| {
                                if let Value::Object(doc_obj) = val {
                                    let doc_map: HashMap<String, Value> = doc_obj
                                        .iter()
                                        .map(|(k, v)| (k.clone(), v.clone()))
                                        .collect();
                                    match self.json_record_to_bson(&doc_map) {
                                        ROk(d) => Some(d),
                                        RErr(_) => None,
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if !documents.is_empty() {
                            match collection.insert_many(documents.clone()).await {
                                Ok(_) => insert_count += documents.len() as i32,
                                Err(e) => {
                                    return RErr(RBoxError::from_fmt(&format_args!(
                                        "bulkWrite insertMany failed: {}",
                                        e
                                    )))
                                }
                            }
                        }
                    }
                }
                "updateOne" => {
                    let filter = match op_obj.get("filter") {
                        Some(Value::Object(f)) => {
                            let mut filter_doc = Document::new();
                            for (key, value) in f {
                                if let Some(bson_val) = json_to_bson(value) {
                                    filter_doc.insert(key.clone(), bson_val);
                                }
                            }
                            filter_doc
                        }
                        _ => Document::new(),
                    };

                    if let Some(Value::Object(update_obj)) = op_obj.get("update") {
                        let update_map: HashMap<String, Value> = update_obj
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let update_doc = match self.json_record_to_bson(&update_map) {
                            ROk(d) => d,
                            RErr(e) => return RErr(e),
                        };
                        let update = mongodb::bson::doc! { "$set": update_doc };
                        match collection.update_one(filter, update).await {
                            Ok(_) => update_count += 1,
                            Err(e) => {
                                return RErr(RBoxError::from_fmt(&format_args!(
                                    "bulkWrite updateOne failed: {}",
                                    e
                                )))
                            }
                        }
                    }
                }
                "updateMany" => {
                    let filter = match op_obj.get("filter") {
                        Some(Value::Object(f)) => {
                            let mut filter_doc = Document::new();
                            for (key, value) in f {
                                if let Some(bson_val) = json_to_bson(value) {
                                    filter_doc.insert(key.clone(), bson_val);
                                }
                            }
                            filter_doc
                        }
                        _ => Document::new(),
                    };

                    if let Some(Value::Object(update_obj)) = op_obj.get("update") {
                        let update_map: HashMap<String, Value> = update_obj
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let update_doc = match self.json_record_to_bson(&update_map) {
                            ROk(d) => d,
                            RErr(e) => return RErr(e),
                        };
                        let update = mongodb::bson::doc! { "$set": update_doc };
                        match collection.update_many(filter, update).await {
                            Ok(result) => update_count += result.modified_count as i32,
                            Err(e) => {
                                return RErr(RBoxError::from_fmt(&format_args!(
                                    "bulkWrite updateMany failed: {}",
                                    e
                                )))
                            }
                        }
                    }
                }
                "deleteOne" => {
                    let filter = match op_obj.get("filter") {
                        Some(Value::Object(f)) => {
                            let mut filter_doc = Document::new();
                            for (key, value) in f {
                                if let Some(bson_val) = json_to_bson(value) {
                                    filter_doc.insert(key.clone(), bson_val);
                                }
                            }
                            filter_doc
                        }
                        _ => Document::new(),
                    };

                    match collection.delete_one(filter).await {
                        Ok(_) => delete_count += 1,
                        Err(e) => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "bulkWrite deleteOne failed: {}",
                                e
                            )))
                        }
                    }
                }
                "deleteMany" => {
                    let filter = match op_obj.get("filter") {
                        Some(Value::Object(f)) => {
                            let mut filter_doc = Document::new();
                            for (key, value) in f {
                                if let Some(bson_val) = json_to_bson(value) {
                                    filter_doc.insert(key.clone(), bson_val);
                                }
                            }
                            filter_doc
                        }
                        _ => Document::new(),
                    };

                    match collection.delete_many(filter).await {
                        Ok(result) => delete_count += result.deleted_count as i32,
                        Err(e) => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "bulkWrite deleteMany failed: {}",
                                e
                            )))
                        }
                    }
                }
                "replaceOne" => {
                    let filter = match op_obj.get("filter") {
                        Some(Value::Object(f)) => {
                            let mut filter_doc = Document::new();
                            for (key, value) in f {
                                if let Some(bson_val) = json_to_bson(value) {
                                    filter_doc.insert(key.clone(), bson_val);
                                }
                            }
                            filter_doc
                        }
                        _ => Document::new(),
                    };

                    if let Some(Value::Object(replacement_obj)) = op_obj.get("replacement") {
                        let replacement_map: HashMap<String, Value> = replacement_obj
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let replacement = match self.json_record_to_bson(&replacement_map) {
                            ROk(d) => d,
                            RErr(e) => return RErr(e),
                        };
                        match collection.replace_one(filter, replacement).await {
                            Ok(_) => replace_count += 1,
                            Err(e) => {
                                return RErr(RBoxError::from_fmt(&format_args!(
                                    "bulkWrite replaceOne failed: {}",
                                    e
                                )))
                            }
                        }
                    }
                }
                _ => {
                    return RErr(RBoxError::from_fmt(&format_args!(
                        "Unsupported operation type '{}'. Supported types: insertOne, insertMany, updateOne, updateMany, deleteOne, deleteMany, replaceOne",
                        op_type
                    )))
                }
            }
        }

        // Return summary as JSON records
        let summary = vec![HashMap::from([
            ("inserted".to_string(), Value::Number(insert_count.into())),
            ("updated".to_string(), Value::Number(update_count.into())),
            ("deleted".to_string(), Value::Number(delete_count.into())),
            ("replaced".to_string(), Value::Number(replace_count.into())),
        ])];

        FfiDataFormat::from_json_records(&summary)
    }

    /// Execute aggregation operation - run aggregation pipeline
    async fn execute_aggregate_async(
        &self,
        config: &HashMap<String, String>,
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<FfiDataFormat, RBoxError> {
        let (client, db_name, collection_name) = match self.connect_mongodb(config, input_data).await {
            ROk(conn) => conn,
            RErr(e) => return RErr(e),
        };
        let db = client.database(&db_name);
        let collection = db.collection::<Document>(&collection_name);

        // Parse pipeline from config with template support
        let pipeline_str = match self.render_config_value(config, "pipeline", input_data) {
            ROk(Some(p)) => p,
            ROk(None) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'pipeline' configuration"
                )))
            }
            RErr(e) => return RErr(e),
        };

        // Parse pipeline as JSON array
        let pipeline_json: Vec<Value> = match serde_json::from_str(&pipeline_str) {
            Ok(Value::Array(arr)) => arr,
            _ => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Pipeline must be a JSON array"
                )))
            }
        };

        // Convert JSON pipeline to BSON documents
        let pipeline: Vec<Document> = pipeline_json
            .iter()
            .filter_map(|stage| {
                if let Value::Object(obj) = stage {
                    let mut doc = Document::new();
                    for (key, value) in obj {
                        if let Some(bson_val) = json_to_bson(value) {
                            doc.insert(key.clone(), bson_val);
                        }
                    }
                    Some(doc)
                } else {
                    None
                }
            })
            .collect();

        if pipeline.is_empty() {
            return RErr(RBoxError::from_fmt(&format_args!(
                "Pipeline cannot be empty"
            )));
        }

        // Execute aggregation
        let mut cursor = match collection.aggregate(pipeline).await {
            Ok(c) => c,
            Err(e) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "MongoDB aggregation failed: {}",
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
                        "Failed to fetch aggregation result: {}",
                        e
                    )))
                }
            }
        }

        FfiDataFormat::from_json_records(&records)
    }

    // Helper methods

    /// Connect to MongoDB and return client, database name, and collection name
    async fn connect_mongodb(
        &self,
        config: &HashMap<String, String>,
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<(Client, String, String), RBoxError> {
        let uri = match self.render_config_value(config, "uri", input_data) {
            ROk(Some(u)) => u,
            ROk(None) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'uri' configuration"
                )))
            }
            RErr(e) => return RErr(e),
        };

        let database = match self.render_config_value(config, "database", input_data) {
            ROk(Some(d)) => d,
            ROk(None) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'database' configuration"
                )))
            }
            RErr(e) => return RErr(e),
        };

        let collection = match self.render_config_value(config, "collection", input_data) {
            ROk(Some(c)) => c,
            ROk(None) => {
                return RErr(RBoxError::from_fmt(&format_args!(
                    "Missing required 'collection' configuration"
                )))
            }
            RErr(e) => return RErr(e),
        };

        let client_options = match ClientOptions::parse(&uri).await {
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

        ROk((client, database, collection))
    }

    /// Parse query from config with template support
    fn parse_query(
        &self,
        config: &HashMap<String, String>,
        input_data: Option<&FfiDataFormat>,
    ) -> RResult<Document, RBoxError> {
        let query_str = match self.render_config_value(config, "query", input_data) {
            ROk(Some(q)) => q,
            ROk(None) => return ROk(Document::new()),
            RErr(e) => return RErr(e),
        };

        match serde_json::from_str::<Value>(&query_str) {
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

    /// Parse document from config (for direct document specification)
    fn parse_document_from_config(&self, config: &HashMap<String, String>) -> RResult<Option<Document>, RBoxError> {
        if let Some(doc_str) = config.get("document") {
            match serde_json::from_str::<Value>(doc_str) {
                Ok(Value::Object(obj)) => {
                    let record: HashMap<String, Value> = obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    match self.json_record_to_bson(&record) {
                        ROk(doc) => ROk(Some(doc)),
                        RErr(e) => RErr(e),
                    }
                }
                Ok(Value::Array(_)) => {
                    // For insertMany, return error - should use documents (plural)
                    RErr(RBoxError::from_fmt(&format_args!(
                        "For multiple documents, use 'documents' field instead of 'document'"
                    )))
                }
                _ => RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid document format in config"
                ))),
            }
        } else {
            ROk(None)
        }
    }

    /// Parse documents array from config (for insertMany)
    fn parse_documents_from_config(&self, config: &HashMap<String, String>) -> RResult<Option<Vec<Document>>, RBoxError> {
        if let Some(docs_str) = config.get("documents") {
            match serde_json::from_str::<Value>(docs_str) {
                Ok(Value::Array(arr)) => {
                    let mut documents = Vec::new();
                    for item in arr {
                        if let Value::Object(obj) = item {
                            let record: HashMap<String, Value> = obj
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();
                            match self.json_record_to_bson(&record) {
                                ROk(doc) => documents.push(doc),
                                RErr(e) => return RErr(e),
                            }
                        }
                    }
                    if documents.is_empty() {
                        RErr(RBoxError::from_fmt(&format_args!(
                            "No valid documents found in config"
                        )))
                    } else {
                        ROk(Some(documents))
                    }
                }
                _ => RErr(RBoxError::from_fmt(&format_args!(
                    "documents field must be a JSON array"
                ))),
            }
        } else {
            ROk(None)
        }
    }

    /// Parse update document from config (for update operations)
    fn parse_update_from_config(&self, config: &HashMap<String, String>) -> RResult<Option<Document>, RBoxError> {
        if let Some(update_str) = config.get("update") {
            match serde_json::from_str::<Value>(update_str) {
                Ok(Value::Object(obj)) => {
                    let record: HashMap<String, Value> = obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    match self.json_record_to_bson(&record) {
                        ROk(doc) => ROk(Some(doc)),
                        RErr(e) => RErr(e),
                    }
                }
                _ => RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid update format in config"
                ))),
            }
        } else {
            ROk(None)
        }
    }

    /// Parse replacement document from config (for replace operations)
    fn parse_replacement_from_config(&self, config: &HashMap<String, String>) -> RResult<Option<Document>, RBoxError> {
        if let Some(repl_str) = config.get("replacement") {
            match serde_json::from_str::<Value>(repl_str) {
                Ok(Value::Object(obj)) => {
                    let record: HashMap<String, Value> = obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    match self.json_record_to_bson(&record) {
                        ROk(doc) => ROk(Some(doc)),
                        RErr(e) => RErr(e),
                    }
                }
                _ => RErr(RBoxError::from_fmt(&format_args!(
                    "Invalid replacement format in config"
                ))),
            }
        } else {
            ROk(None)
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
            // Get input data if available
            let input_data = context.inputs.into_iter().next().map(|tuple| tuple.1);

            match self.operation {
                MongoOperation::Find => self.execute_find_async(&config, input_data.as_ref()).await,
                MongoOperation::FindOne => self.execute_find_one_async(&config, input_data.as_ref()).await,
                MongoOperation::Aggregate => self.execute_aggregate_async(&config, input_data.as_ref()).await,
                MongoOperation::BulkWrite => {
                    let input_data = match input_data {
                        Some(data) => data,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "bulkWrite requires input data"
                            )))
                        }
                    };
                    self.execute_bulk_write_async(&input_data, &config).await
                }
                MongoOperation::InsertOne => {
                    let input_data = match input_data {
                        Some(data) => data,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "insertOne requires input data"
                            )))
                        }
                    };
                    self.execute_insert_one_async(&input_data, &config).await
                }
                MongoOperation::InsertMany => {
                    let input_data = match input_data {
                        Some(data) => data,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "insertMany requires input data"
                            )))
                        }
                    };
                    self.execute_insert_many_async(&input_data, &config).await
                }
                MongoOperation::UpdateOne => {
                    let input_data = match input_data {
                        Some(data) => data,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "updateOne requires input data"
                            )))
                        }
                    };
                    self.execute_update_one_async(&input_data, &config).await
                }
                MongoOperation::UpdateMany => {
                    let input_data = match input_data {
                        Some(data) => data,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "updateMany requires input data"
                            )))
                        }
                    };
                    self.execute_update_many_async(&input_data, &config).await
                }
                MongoOperation::DeleteOne => self.execute_delete_one_async(&config, input_data.as_ref()).await,
                MongoOperation::DeleteMany => self.execute_delete_many_async(&config, input_data.as_ref()).await,
                MongoOperation::ReplaceOne => {
                    let input_data = match input_data {
                        Some(data) => data,
                        None => {
                            return RErr(RBoxError::from_fmt(&format_args!(
                                "replaceOne requires input data"
                            )))
                        }
                    };
                    self.execute_replace_one_async(&input_data, &config).await
                }
                MongoOperation::ReplaceMany => {
                    let input_data = match input_data {
                        Some(data) => data,
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

#[no_mangle]
pub extern "C" fn create_mongodb_aggregate() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-aggregate".to_string(),
            MongoOperation::Aggregate,
            StageType::Source,
        ),
        TD_Opaque,
    )
}

#[no_mangle]
pub extern "C" fn create_mongodb_bulkwrite() -> FfiStage_TO<'static, RBox<()>> {
    FfiStage_TO::from_value(
        MongoDbStage::new(
            "mongodb-bulkwrite".to_string(),
            MongoOperation::BulkWrite,
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
         Uses cursor-based iteration for efficient large result sets. \
         \n\nSupports Handlebars templates: uri, database, collection, and query fields \
         can use {{ field }} syntax to reference input data fields.",
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
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::optional(
        "document",
        FfiParameterType::String,
        "",
        "Document to insert as JSON string (alternative to input data). Example: '{\"name\": \"John\", \"age\": 30}'",
    ));

    FfiStageMetadata::new(
        "mongodb.insertOne",
        "Insert single document into MongoDB collection",
        "Inserts a single document into MongoDB. \
         Document can be provided via input data (first record) or 'document' config parameter.",
        params,
        vec!["mongodb", "database", "sink", "insert"],
    )
}

/// Create metadata for insertMany operation
fn create_insertmany_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::optional(
        "documents",
        FfiParameterType::String,
        "",
        "Documents to insert as JSON array string (alternative to input data). Example: '[{\"name\": \"John\"}, {\"name\": \"Jane\"}]'",
    ));

    FfiStageMetadata::new(
        "mongodb.insertMany",
        "Insert multiple documents into MongoDB collection",
        "Inserts multiple documents into MongoDB in a single batch operation. \
         Documents can be provided via input data (all records) or 'documents' config parameter. \
         Efficient for bulk inserts.",
        params,
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
    params.push(FfiConfigParameter::optional(
        "update",
        FfiParameterType::String,
        "",
        "Fields to update as JSON string (alternative to input data). Example: '{\"status\": \"active\", \"updated_at\": \"2024-01-01\"}'",
    ));

    FfiStageMetadata::new(
        "mongodb.updateOne",
        "Update single document in MongoDB collection",
        "Updates the first document matching the query filter using $set operator. \
         Update fields can be provided via input data (first record) or 'update' config parameter.",
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
    params.push(FfiConfigParameter::optional(
        "update",
        FfiParameterType::String,
        "",
        "Fields to update as JSON string (alternative to input data). Example: '{\"status\": \"active\", \"updated_at\": \"2024-01-01\"}'",
    ));

    FfiStageMetadata::new(
        "mongodb.updateMany",
        "Update multiple documents in MongoDB collection",
        "Updates all documents matching the query filter using $set operator. \
         Update fields can be provided via input data (first record) or 'update' config parameter.",
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
    params.push(FfiConfigParameter::optional(
        "replacement",
        FfiParameterType::String,
        "",
        "Replacement document as JSON string (alternative to input data). Example: '{\"name\": \"Alice\", \"age\": 30}'",
    ));

    FfiStageMetadata::new(
        "mongodb.replaceOne",
        "Replace single document in MongoDB collection",
        "Replaces the entire document matching the query filter with new document. \
         Unlike update, this replaces the whole document (except _id). \
         Document can be provided via input data (first record) or 'replacement' config parameter.",
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
    params.push(FfiConfigParameter::optional(
        "replacement",
        FfiParameterType::String,
        "",
        "Replacement document as JSON string (alternative to input data). Example: '{\"name\": \"Alice\", \"age\": 30}'",
    ));

    FfiStageMetadata::new(
        "mongodb.replaceMany",
        "Replace multiple documents in MongoDB collection",
        "Replaces all documents matching the query filter with new document. \
         Iterates through matching documents and replaces each one individually. \
         Document can be provided via input data (first record) or 'replacement' config parameter.",
        params,
        vec!["mongodb", "database", "sink", "replace", "bulk"],
    )
}

/// Create metadata for aggregate operation
fn create_aggregate_metadata() -> FfiStageMetadata {
    let mut params = common_mongodb_parameters();
    params.push(FfiConfigParameter::required(
        "pipeline",
        FfiParameterType::String,
        "MongoDB aggregation pipeline as JSON array (e.g., '[{\"$match\": {\"status\": \"active\"}}, {\"$group\": {\"_id\": \"$category\", \"count\": {\"$sum\": 1}}}]')",
    ));

    FfiStageMetadata::new(
        "mongodb.aggregate",
        "Run aggregation pipeline on MongoDB collection",
        "Executes a MongoDB aggregation pipeline and returns results. \
         Supports all MongoDB aggregation stages ($match, $group, $project, $sort, $limit, etc.). \
         Pipeline must be provided as a JSON array of stage objects.",
        params,
        vec!["mongodb", "database", "source", "aggregation", "pipeline"],
    )
}

/// Create metadata for bulkWrite operation
fn create_bulkwrite_metadata() -> FfiStageMetadata {
    FfiStageMetadata::new(
        "mongodb.bulkWrite",
        "Execute multiple write operations in a single batch",
        "Executes multiple write operations in a single batch for efficient bulk processing. \
         Input data must contain an 'operations' array with operation objects. \
         Each operation must have a 'type' field and operation-specific fields. \
         \n\nSupported operation types:\n\
         - insertOne: Insert single document (requires 'document' field)\n\
         - insertMany: Insert multiple documents (requires 'documents' array field)\n\
         - updateOne: Update single document (requires 'filter' and 'update' fields)\n\
         - updateMany: Update multiple documents (requires 'filter' and 'update' fields)\n\
         - deleteOne: Delete single document (requires 'filter' field)\n\
         - deleteMany: Delete multiple documents (requires 'filter' field)\n\
         - replaceOne: Replace single document (requires 'filter' and 'replacement' fields)\n\
         \n\nReturns a summary with counts of inserted, updated, deleted, and replaced documents.",
        common_mongodb_parameters(),
        vec!["mongodb", "database", "sink", "bulk", "write", "batch"],
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
            "mongodb.aggregate",
            StageType::Source,
            "MongoDB aggregate - run aggregation pipeline",
            "create_mongodb_aggregate",
            create_aggregate_metadata(),
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
        PluginCapability::new(
            "mongodb.bulkWrite",
            StageType::Sink,
            "MongoDB bulkWrite - execute multiple write operations",
            "create_mongodb_bulkwrite",
            create_bulkwrite_metadata(),
        ),
    ]
    .into()
}

// Plugin declaration
#[no_mangle]
pub static _plugin_declaration: PluginDeclaration = PluginDeclaration {
    api_version: PLUGIN_API_VERSION,
    name: rstr!("mongodb"),
    version: rstr!("2.2.0"),
    description: rstr!(
        "MongoDB plugin with operation-based API (find, findOne, aggregate, insert, update, delete, replace, bulkWrite)"
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
        assert_eq!(_plugin_declaration.version, "2.2.0");
        assert!(_plugin_declaration.is_compatible());
    }

    #[test]
    fn test_capabilities() {
        let caps = get_capabilities();
        assert_eq!(caps.len(), 12);

        // Check find operation
        assert_eq!(caps[0].name.as_str(), "mongodb.find");
        assert_eq!(caps[0].stage_type, StageType::Source);

        // Check findOne operation
        assert_eq!(caps[1].name.as_str(), "mongodb.findOne");
        assert_eq!(caps[1].stage_type, StageType::Source);

        // Check aggregate operation
        assert_eq!(caps[2].name.as_str(), "mongodb.aggregate");
        assert_eq!(caps[2].stage_type, StageType::Source);

        // Check insertOne operation
        assert_eq!(caps[3].name.as_str(), "mongodb.insertOne");
        assert_eq!(caps[3].stage_type, StageType::Sink);

        // Check insertMany operation
        assert_eq!(caps[4].name.as_str(), "mongodb.insertMany");
        assert_eq!(caps[4].stage_type, StageType::Sink);

        // Check bulkWrite operation
        assert_eq!(caps[11].name.as_str(), "mongodb.bulkWrite");
        assert_eq!(caps[11].stage_type, StageType::Sink);
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

    #[test]
    fn test_parse_document_from_config() {
        let stage = MongoDbStage::new(
            "mongodb.insertOne".to_string(),
            MongoOperation::InsertOne,
            StageType::Sink,
        );

        // Test valid document
        let mut config = HashMap::new();
        config.insert(
            "document".to_string(),
            r#"{"name": "John", "age": 30}"#.to_string(),
        );

        let result = stage.parse_document_from_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // Test invalid JSON
        config.insert("document".to_string(), "invalid json".to_string());
        let result = stage.parse_document_from_config(&config);
        assert!(result.is_err());

        // Test no document field
        config.clear();
        let result = stage.parse_document_from_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_documents_from_config() {
        let stage = MongoDbStage::new(
            "mongodb.insertMany".to_string(),
            MongoOperation::InsertMany,
            StageType::Sink,
        );

        // Test valid documents array
        let mut config = HashMap::new();
        config.insert(
            "documents".to_string(),
            r#"[{"name": "John"}, {"name": "Jane"}]"#.to_string(),
        );

        let result = stage.parse_documents_from_config(&config);
        assert!(result.is_ok());
        let docs = result.unwrap();
        assert!(docs.is_some());
        assert_eq!(docs.unwrap().len(), 2);

        // Test empty array
        config.insert("documents".to_string(), "[]".to_string());
        let result = stage.parse_documents_from_config(&config);
        assert!(result.is_err());

        // Test no documents field
        config.clear();
        let result = stage.parse_documents_from_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_update_from_config() {
        let stage = MongoDbStage::new(
            "mongodb.updateOne".to_string(),
            MongoOperation::UpdateOne,
            StageType::Sink,
        );

        // Test valid update
        let mut config = HashMap::new();
        config.insert(
            "update".to_string(),
            r#"{"status": "active", "updated_at": "2024-01-01"}"#.to_string(),
        );

        let result = stage.parse_update_from_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // Test no update field
        config.clear();
        let result = stage.parse_update_from_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_replacement_from_config() {
        let stage = MongoDbStage::new(
            "mongodb.replaceOne".to_string(),
            MongoOperation::ReplaceOne,
            StageType::Sink,
        );

        // Test valid replacement
        let mut config = HashMap::new();
        config.insert(
            "replacement".to_string(),
            r#"{"name": "Alice", "age": 30}"#.to_string(),
        );

        let result = stage.parse_replacement_from_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // Test no replacement field
        config.clear();
        let result = stage.parse_replacement_from_config(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
