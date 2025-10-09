pub mod sinks;
pub mod sources;
pub mod stages;
pub mod transforms;

use crate::core::stage::StageRef;
use std::collections::HashMap;
use std::sync::Arc;

/// Register all built-in modules as functions
///
/// Function-based API where each operation is a named function:
/// - csv.read, csv.write
/// - json.read, json.write
/// - filter.apply, map.apply, etc.
pub fn register_functions() -> HashMap<String, StageRef> {
    let mut functions = HashMap::new();

    // CSV functions
    functions.insert(
        "csv.read".to_string(),
        Arc::new(sources::csv::CsvSource) as StageRef,
    );
    functions.insert(
        "csv.write".to_string(),
        Arc::new(sinks::csv::CsvSink) as StageRef,
    );

    // JSON functions
    functions.insert(
        "json.read".to_string(),
        Arc::new(sources::json::JsonSource) as StageRef,
    );
    functions.insert(
        "json.write".to_string(),
        Arc::new(sinks::json::JsonSink) as StageRef,
    );

    // Stdin/Stdout functions
    functions.insert(
        "stdin.read".to_string(),
        Arc::new(sources::stdin::StdinSource) as StageRef,
    );
    functions.insert(
        "stdout.write".to_string(),
        Arc::new(sinks::stdout::StdoutSink) as StageRef,
    );
    functions.insert(
        "stdout_stream.write".to_string(),
        Arc::new(sinks::stdout_stream::StdoutStreamSink::new()) as StageRef,
    );

    // Transform functions
    functions.insert(
        "decrypt.apply".to_string(),
        Arc::new(transforms::decrypt::DecryptTransform) as StageRef,
    );
    functions.insert(
        "encrypt.apply".to_string(),
        Arc::new(transforms::encrypt::EncryptTransform) as StageRef,
    );
    functions.insert(
        "filter.apply".to_string(),
        Arc::new(transforms::filter::FilterTransform) as StageRef,
    );
    functions.insert(
        "map.apply".to_string(),
        Arc::new(transforms::map::MapTransform) as StageRef,
    );
    functions.insert(
        "validate.schema".to_string(),
        Arc::new(transforms::validate::ValidateSchemaTransform) as StageRef,
    );
    functions.insert(
        "http.fetch".to_string(),
        Arc::new(transforms::http_fetch::HttpFetchTransform::new()) as StageRef,
    );
    functions.insert(
        "reduce.apply".to_string(),
        Arc::new(transforms::reduce::ReduceTransform) as StageRef,
    );
    functions.insert(
        "groupby.apply".to_string(),
        Arc::new(transforms::group_by::GroupByTransform) as StageRef,
    );
    functions.insert(
        "sort.apply".to_string(),
        Arc::new(transforms::sort::SortTransform) as StageRef,
    );
    functions.insert(
        "select.apply".to_string(),
        Arc::new(transforms::select::SelectTransform) as StageRef,
    );
    functions.insert(
        "distinct.apply".to_string(),
        Arc::new(transforms::distinct::DistinctTransform) as StageRef,
    );
    functions.insert(
        "window.apply".to_string(),
        Arc::new(transforms::window::WindowTransform::new()) as StageRef,
    );
    functions.insert(
        "aggregate.stream".to_string(),
        Arc::new(transforms::aggregate_stream::AggregateStreamTransform::new()) as StageRef,
    );
    functions.insert(
        "json.extract".to_string(),
        Arc::new(transforms::json_extract::JsonExtractTransform) as StageRef,
    );
    functions.insert(
        "ai.generate".to_string(),
        Arc::new(transforms::ai::AiGenerateTransform::new()) as StageRef,
    );

    functions
}
