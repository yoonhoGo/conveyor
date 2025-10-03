pub mod aggregate_stream;
pub mod distinct;
pub mod filter;
pub mod group_by;
pub mod http_fetch;
pub mod map;
pub mod reduce;
pub mod select;
pub mod sort;
pub mod validate;
pub mod window;

use crate::core::traits::TransformRef;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_transforms() -> HashMap<String, TransformRef> {
    let mut transforms = HashMap::new();

    transforms.insert(
        "filter".to_string(),
        Arc::new(filter::FilterTransform) as TransformRef,
    );
    transforms.insert(
        "map".to_string(),
        Arc::new(map::MapTransform) as TransformRef,
    );
    transforms.insert(
        "validate_schema".to_string(),
        Arc::new(validate::ValidateSchemaTransform) as TransformRef,
    );
    transforms.insert(
        "http_fetch".to_string(),
        Arc::new(http_fetch::HttpFetchTransform::new()) as TransformRef,
    );
    transforms.insert(
        "reduce".to_string(),
        Arc::new(reduce::ReduceTransform) as TransformRef,
    );
    transforms.insert(
        "group_by".to_string(),
        Arc::new(group_by::GroupByTransform) as TransformRef,
    );
    transforms.insert(
        "sort".to_string(),
        Arc::new(sort::SortTransform) as TransformRef,
    );
    transforms.insert(
        "select".to_string(),
        Arc::new(select::SelectTransform) as TransformRef,
    );
    transforms.insert(
        "distinct".to_string(),
        Arc::new(distinct::DistinctTransform) as TransformRef,
    );
    transforms.insert(
        "window".to_string(),
        Arc::new(window::WindowTransform::new()) as TransformRef,
    );
    transforms.insert(
        "aggregate_stream".to_string(),
        Arc::new(aggregate_stream::AggregateStreamTransform::new()) as TransformRef,
    );

    transforms
}
