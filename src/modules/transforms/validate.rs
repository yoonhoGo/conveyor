use anyhow::Result;
use async_trait::async_trait;
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct ValidateSchemaTransform;

#[async_trait]
impl Stage for ValidateSchemaTransform {
    fn name(&self) -> &str {
        "validate_schema"
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Validate transform requires input data"))?;

        let df = data.as_dataframe()?;

        // Check required fields
        if let Some(required_fields) = config.get("required_fields") {
            if let Some(fields) = required_fields.as_array() {
                let column_names = df.get_column_names();

                for field_value in fields {
                    if let Some(field) = field_value.as_str() {
                        if !column_names.iter().any(|name| name.as_str() == field) {
                            anyhow::bail!("Required field '{}' not found in data", field);
                        }
                    }
                }
            }
        }

        // Validate data types
        if let Some(field_types) = config.get("field_types") {
            if let Some(types_table) = field_types.as_table() {
                for (field, expected_type) in types_table {
                    if let Some(expected_type_str) = expected_type.as_str() {
                        if let Ok(column) = df.column(field) {
                            let actual_type = column.dtype();
                            if !validate_type(actual_type, expected_type_str)? {
                                anyhow::bail!(
                                    "Field '{}' has type {:?}, expected {}",
                                    field,
                                    actual_type,
                                    expected_type_str
                                );
                            }
                        }
                    }
                }
            }
        }

        // Check for nulls in non-nullable fields
        if let Some(non_nullable) = config.get("non_nullable") {
            if let Some(fields) = non_nullable.as_array() {
                for field_value in fields {
                    if let Some(field) = field_value.as_str() {
                        if let Ok(column) = df.column(field) {
                            if column.null_count() > 0 {
                                anyhow::bail!("Field '{}' contains null values", field);
                            }
                        }
                    }
                }
            }
        }

        // Validate date fields
        if let Some(date_fields) = config.get("date_fields") {
            if let Some(fields) = date_fields.as_array() {
                for field_value in fields {
                    if let Some(field) = field_value.as_str() {
                        if let Ok(column) = df.column(field) {
                            // Check if it's a valid date/datetime column
                            match column.dtype() {
                                DataType::Date | DataType::Datetime(_, _) => {
                                    // Already in correct format
                                }
                                DataType::String => {
                                    // Try to parse as date - this is a validation check
                                    // In a real implementation, you might want to actually convert
                                    tracing::warn!(
                                        "Field '{}' is string but expected to be date",
                                        field
                                    );
                                }
                                _ => {
                                    anyhow::bail!("Field '{}' is not a valid date field", field);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check unique constraints
        if let Some(unique_fields) = config.get("unique_fields") {
            if let Some(fields) = unique_fields.as_array() {
                for field_value in fields {
                    if let Some(field) = field_value.as_str() {
                        if let Ok(column) = df.column(field) {
                            let series = column.as_materialized_series();
                            let unique_count = series.n_unique()?;
                            let total_count = series.len();
                            if unique_count != total_count {
                                anyhow::bail!(
                                    "Field '{}' has duplicate values ({} unique out of {})",
                                    field,
                                    unique_count,
                                    total_count
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(data)
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if config.is_empty() {
            anyhow::bail!("Validate schema transform requires configuration");
        }

        // Configuration is validated when applied
        Ok(())
    }
}

fn validate_type(actual: &DataType, expected: &str) -> Result<bool> {
    let matches = match expected.to_lowercase().as_str() {
        "string" | "str" | "text" => matches!(actual, DataType::String),
        "int" | "integer" | "int64" | "i64" => matches!(
            actual,
            DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64
        ),
        "float" | "double" | "f64" | "float64" => {
            matches!(actual, DataType::Float32 | DataType::Float64)
        }
        "bool" | "boolean" => matches!(actual, DataType::Boolean),
        "date" => matches!(actual, DataType::Date),
        "datetime" | "timestamp" => matches!(actual, DataType::Datetime(_, _)),
        _ => {
            anyhow::bail!("Unknown expected type: {}", expected);
        }
    };

    Ok(matches)
}
