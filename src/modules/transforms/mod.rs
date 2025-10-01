pub mod filter;
pub mod map;
pub mod validate;

use crate::core::traits::TransformRef;
use std::collections::HashMap;
use std::sync::Arc;

pub fn register_transforms() -> HashMap<String, TransformRef> {
    let mut transforms = HashMap::new();

    transforms.insert("filter".to_string(), Arc::new(filter::FilterTransform) as TransformRef);
    transforms.insert("map".to_string(), Arc::new(map::MapTransform) as TransformRef);
    transforms.insert("validate_schema".to_string(), Arc::new(validate::ValidateSchemaTransform) as TransformRef);

    transforms
}