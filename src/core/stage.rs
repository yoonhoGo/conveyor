use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::traits::DataFormat;
use crate::wasm_plugin_loader::{
    DataFormat as WasmDataFormat, ExecutionContext as WasmExecutionContext, WasmPluginLoader,
};
use conveyor_plugin_api::traits::FfiStage_TO;
use conveyor_plugin_api::{FfiDataFormat, FfiExecutionContext, RBox, RHashMap, RString};

/// Unified Stage trait - represents any processing unit in the pipeline
/// This allows sources, transforms, and sinks to be treated uniformly in a DAG
#[async_trait]
pub trait Stage: Send + Sync {
    /// Unique identifier for this stage type
    fn name(&self) -> &str;

    /// Execute the stage with given inputs and configuration
    ///
    /// # Arguments
    /// * `inputs` - Map of input data from previous stages (can be empty for sources)
    /// * `config` - Stage-specific configuration from TOML
    ///
    /// # Returns
    /// * `Result<DataFormat>` - Processed data to pass to next stages
    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat>;

    /// Validate the stage configuration
    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()>;

    /// Whether this stage produces output (false for sinks)
    fn produces_output(&self) -> bool {
        true
    }
}

pub type StageRef = Arc<dyn Stage>;

// ============================================================================
// FFI Plugin Stage Adapter
// ============================================================================

/// Adapter to use FFI plugins as stages
pub struct FfiPluginStageAdapter {
    name: String,
    stage_name: String,
    stage_instance: FfiStage_TO<'static, RBox<()>>,
}

impl FfiPluginStageAdapter {
    pub fn new(
        name: String,
        stage_name: String,
        stage_instance: FfiStage_TO<'static, RBox<()>>,
    ) -> Self {
        Self {
            name,
            stage_name,
            stage_instance,
        }
    }
}

#[async_trait]
impl Stage for FfiPluginStageAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        // Convert inputs to FFI format
        let mut ffi_inputs = RHashMap::new();
        for (key, value) in inputs {
            let ffi_data = dataformat_to_ffi(&value)?;
            ffi_inputs.insert(RString::from(key), ffi_data);
        }

        // Convert config to FFI format
        let ffi_config = config_to_ffi(config)?;

        // Create execution context
        let context = FfiExecutionContext::new(ffi_inputs, ffi_config);

        // Execute FFI stage (synchronous call)
        let result = self.stage_instance.execute(context);

        // Convert result back
        match result {
            conveyor_plugin_api::ROk(ffi_data) => {
                let data = ffi_to_dataformat(&ffi_data)?;
                Ok(data)
            }
            conveyor_plugin_api::RErr(e) => Err(anyhow::anyhow!(
                "FFI plugin '{}' error: {:?}",
                self.stage_name,
                e
            )),
        }
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        let ffi_config = config_to_ffi(config)?;

        match self.stage_instance.validate_config(ffi_config) {
            conveyor_plugin_api::ROk(_) => Ok(()),
            conveyor_plugin_api::RErr(e) => Err(anyhow::anyhow!(
                "FFI plugin '{}' config validation error: {:?}",
                self.stage_name,
                e
            )),
        }
    }
}

// ============================================================================
// WASM Plugin Stage Adapter
// ============================================================================

/// Adapter to use WASM plugins as stages
pub struct WasmPluginStageAdapter {
    name: String,
    plugin_name: String,
    stage_name: String,
    loader: Arc<WasmPluginLoader>,
}

impl WasmPluginStageAdapter {
    pub fn new(
        name: String,
        plugin_name: String,
        stage_name: String,
        loader: Arc<WasmPluginLoader>,
    ) -> Self {
        Self {
            name,
            plugin_name,
            stage_name,
            loader,
        }
    }
}

#[async_trait]
impl Stage for WasmPluginStageAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        // Convert inputs to WASM format
        let mut wasm_inputs = Vec::new();
        for (key, value) in inputs {
            let wasm_data = dataformat_to_wasm(&value)?;
            wasm_inputs.push((key, wasm_data));
        }

        // Convert config to WASM format
        let wasm_config = config_to_wasm(config)?;

        // Create execution context
        let context = WasmExecutionContext {
            inputs: wasm_inputs,
            config: wasm_config,
        };

        // Execute WASM stage (async)
        let wasm_result = self
            .loader
            .execute(&self.plugin_name, &self.stage_name, context)
            .await?;

        // Convert result back
        let data = wasm_to_dataformat(&wasm_result)?;
        Ok(data)
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        let wasm_config = config_to_wasm(config)?;

        self.loader
            .validate_config(&self.plugin_name, &self.stage_name, wasm_config)
            .await
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Convert DataFormat to FfiDataFormat
fn dataformat_to_ffi(data: &DataFormat) -> Result<FfiDataFormat> {
    match data {
        DataFormat::DataFrame(df) => {
            // Serialize DataFrame to Arrow IPC
            let mut buf = Vec::new();
            use polars::io::ipc::IpcWriter;
            use polars::prelude::SerWriter;
            let mut df_clone = df.clone();
            IpcWriter::new(&mut buf).finish(&mut df_clone)?;
            Ok(FfiDataFormat::from_arrow_ipc(buf))
        }
        DataFormat::RecordBatch(records) => {
            // Serialize records to JSON
            match FfiDataFormat::from_json_records(records) {
                conveyor_plugin_api::ROk(ffi_data) => Ok(ffi_data),
                conveyor_plugin_api::RErr(e) => {
                    Err(anyhow::anyhow!("Failed to serialize records: {:?}", e))
                }
            }
        }
        DataFormat::Raw(bytes) => Ok(FfiDataFormat::from_raw(bytes.clone())),
        DataFormat::Stream(_) => {
            anyhow::bail!(
                "Cannot convert streaming data to FFI format. Streams must be collected first."
            )
        }
    }
}

/// Convert FfiDataFormat to DataFormat
fn ffi_to_dataformat(ffi_data: &FfiDataFormat) -> Result<DataFormat> {
    match ffi_data {
        FfiDataFormat::ArrowIpc(bytes) => {
            // Deserialize Arrow IPC to DataFrame
            use polars::io::ipc::IpcReader;
            use polars::prelude::SerReader;
            use std::io::Cursor;
            let cursor = Cursor::new(bytes.as_slice());
            let df = IpcReader::new(cursor).finish()?;
            Ok(DataFormat::DataFrame(df))
        }
        FfiDataFormat::JsonRecords(bytes) => {
            // Deserialize JSON to records
            let records: Vec<HashMap<String, serde_json::Value>> =
                serde_json::from_slice(bytes.as_slice())?;
            Ok(DataFormat::RecordBatch(records))
        }
        FfiDataFormat::Raw(bytes) => Ok(DataFormat::Raw(bytes.to_vec())),
    }
}

/// Convert config HashMap to FFI RHashMap
fn config_to_ffi(config: &HashMap<String, toml::Value>) -> Result<RHashMap<RString, RString>> {
    let mut ffi_config = RHashMap::new();
    for (key, value) in config {
        let value_str = match value {
            toml::Value::String(s) => s.clone(),
            toml::Value::Integer(i) => i.to_string(),
            toml::Value::Float(f) => f.to_string(),
            toml::Value::Boolean(b) => b.to_string(),
            other => other.to_string(),
        };
        ffi_config.insert(RString::from(key.clone()), RString::from(value_str));
    }
    Ok(ffi_config)
}

/// Convert Polars AnyValue to serde_json::Value
fn anyvalue_to_json(value: &polars::prelude::AnyValue) -> serde_json::Value {
    use polars::prelude::AnyValue;
    use serde_json::Value as JsonValue;

    match value {
        AnyValue::Null => JsonValue::Null,
        AnyValue::Boolean(b) => JsonValue::Bool(*b),
        AnyValue::Int8(i) => JsonValue::Number((*i).into()),
        AnyValue::Int16(i) => JsonValue::Number((*i).into()),
        AnyValue::Int32(i) => JsonValue::Number((*i).into()),
        AnyValue::Int64(i) => JsonValue::Number((*i).into()),
        AnyValue::UInt8(i) => JsonValue::Number((*i).into()),
        AnyValue::UInt16(i) => JsonValue::Number((*i).into()),
        AnyValue::UInt32(i) => JsonValue::Number((*i).into()),
        AnyValue::UInt64(i) => JsonValue::Number((*i).into()),
        AnyValue::Float32(f) => serde_json::Number::from_f64(*f as f64)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        AnyValue::Float64(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        AnyValue::String(s) => JsonValue::String(s.to_string()),
        AnyValue::StringOwned(s) => JsonValue::String(s.to_string()),
        _ => JsonValue::String(format!("{:?}", value)),
    }
}

/// Convert DataFormat to WASM DataFormat
fn dataformat_to_wasm(data: &DataFormat) -> Result<WasmDataFormat> {
    match data {
        DataFormat::DataFrame(df) => {
            // Convert DataFrame to JSON records for WASM plugins
            use serde_json::Value as JsonValue;
            use std::collections::HashMap;

            let mut records: Vec<HashMap<String, JsonValue>> = Vec::new();
            let height = df.height();

            for row_idx in 0..height {
                let mut record = HashMap::new();
                for col in df.get_columns() {
                    let col_name = col.name().to_string();
                    let value = col.get(row_idx)?;
                    let json_value = anyvalue_to_json(&value);
                    record.insert(col_name, json_value);
                }
                records.push(record);
            }

            let bytes = serde_json::to_vec(&records)?;
            Ok(WasmDataFormat::JsonRecords(bytes))
        }
        DataFormat::RecordBatch(records) => {
            // Serialize records to JSON
            let bytes = serde_json::to_vec(records)?;
            Ok(WasmDataFormat::JsonRecords(bytes))
        }
        DataFormat::Raw(bytes) => Ok(WasmDataFormat::Raw(bytes.clone())),
        DataFormat::Stream(_) => {
            anyhow::bail!(
                "Cannot convert streaming data to WASM format. Streams must be collected first."
            )
        }
    }
}

/// Convert WASM DataFormat to DataFormat
fn wasm_to_dataformat(wasm_data: &WasmDataFormat) -> Result<DataFormat> {
    match wasm_data {
        WasmDataFormat::ArrowIpc(bytes) => {
            // Deserialize Arrow IPC to DataFrame
            use polars::io::ipc::IpcReader;
            use polars::prelude::SerReader;
            use std::io::Cursor;
            let cursor = Cursor::new(bytes.as_slice());
            let df = IpcReader::new(cursor).finish()?;
            Ok(DataFormat::DataFrame(df))
        }
        WasmDataFormat::JsonRecords(bytes) => {
            // Deserialize JSON to records
            let records: Vec<HashMap<String, serde_json::Value>> = serde_json::from_slice(bytes)?;
            Ok(DataFormat::RecordBatch(records))
        }
        WasmDataFormat::Raw(bytes) => Ok(DataFormat::Raw(bytes.clone())),
    }
}

/// Convert config HashMap to WASM format
fn config_to_wasm(config: &HashMap<String, toml::Value>) -> Result<Vec<(String, String)>> {
    let mut wasm_config = Vec::new();
    for (key, value) in config {
        let value_str = match value {
            toml::Value::String(s) => s.clone(),
            toml::Value::Integer(i) => i.to_string(),
            toml::Value::Float(f) => f.to_string(),
            toml::Value::Boolean(b) => b.to_string(),
            other => other.to_string(),
        };
        wasm_config.push((key.clone(), value_str));
    }
    Ok(wasm_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::DataFormat;
    use polars::prelude::DataFrame;

    struct MockStage;

    #[async_trait]
    impl Stage for MockStage {
        fn name(&self) -> &str {
            "mock"
        }

        async fn execute(
            &self,
            _inputs: HashMap<String, DataFormat>,
            _config: &HashMap<String, toml::Value>,
        ) -> Result<DataFormat> {
            Ok(DataFormat::DataFrame(DataFrame::empty()))
        }

        async fn validate_config(&self, _config: &HashMap<String, toml::Value>) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_stage() {
        let stage = MockStage;

        assert_eq!(stage.name(), "mock");
        assert!(stage.produces_output());

        let inputs = HashMap::new();
        let config = HashMap::new();
        let result = stage.execute(inputs, &config).await;
        assert!(result.is_ok());
    }
}
