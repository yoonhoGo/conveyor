pub mod csv;
pub mod json;
pub mod stdin;

use crate::core::traits::DataSourceRef;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_sources() -> HashMap<String, DataSourceRef> {
    let mut sources = HashMap::new();

    sources.insert("csv".to_string(), Arc::new(csv::CsvSource) as DataSourceRef);
    sources.insert("json".to_string(), Arc::new(json::JsonSource) as DataSourceRef);
    sources.insert("stdin".to_string(), Arc::new(stdin::StdinSource) as DataSourceRef);

    sources
}