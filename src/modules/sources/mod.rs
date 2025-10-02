pub mod csv;
pub mod http;
pub mod json;
pub mod mongodb;
pub mod stdin;

use crate::core::traits::DataSourceRef;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_sources() -> HashMap<String, DataSourceRef> {
    let mut sources = HashMap::new();

    sources.insert("csv".to_string(), Arc::new(csv::CsvSource) as DataSourceRef);
    sources.insert("http".to_string(), Arc::new(http::HttpSource) as DataSourceRef);
    sources.insert("json".to_string(), Arc::new(json::JsonSource) as DataSourceRef);
    sources.insert("mongodb".to_string(), Arc::new(mongodb::MongodbSource) as DataSourceRef);
    sources.insert("stdin".to_string(), Arc::new(stdin::StdinSource) as DataSourceRef);

    sources
}