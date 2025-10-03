pub mod csv;
pub mod json;
pub mod stdout;
pub mod stdout_stream;

use crate::core::traits::SinkRef;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_sinks() -> HashMap<String, SinkRef> {
    let mut sinks = HashMap::new();

    sinks.insert("csv".to_string(), Arc::new(csv::CsvSink) as SinkRef);
    sinks.insert("json".to_string(), Arc::new(json::JsonSink) as SinkRef);
    sinks.insert(
        "stdout".to_string(),
        Arc::new(stdout::StdoutSink) as SinkRef,
    );
    sinks.insert(
        "stdout_stream".to_string(),
        Arc::new(stdout_stream::StdoutStreamSink::new()) as SinkRef,
    );

    sinks
}
