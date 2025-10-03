pub mod csv;
pub mod file_watch;
pub mod json;
pub mod stdin;
pub mod stdin_stream;

use crate::core::traits::{DataSourceRef, StreamingDataSourceRef};
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_sources() -> HashMap<String, DataSourceRef> {
    let mut sources = HashMap::new();

    sources.insert("csv".to_string(), Arc::new(csv::CsvSource) as DataSourceRef);
    sources.insert(
        "json".to_string(),
        Arc::new(json::JsonSource) as DataSourceRef,
    );
    sources.insert(
        "stdin".to_string(),
        Arc::new(stdin::StdinSource) as DataSourceRef,
    );

    sources
}

pub fn register_streaming_sources() -> HashMap<String, StreamingDataSourceRef> {
    let mut sources = HashMap::new();

    sources.insert(
        "stdin_stream".to_string(),
        Arc::new(stdin_stream::StdinStreamSource::new()) as StreamingDataSourceRef,
    );
    sources.insert(
        "file_watch".to_string(),
        Arc::new(file_watch::FileWatchSource::new()) as StreamingDataSourceRef,
    );

    sources
}
