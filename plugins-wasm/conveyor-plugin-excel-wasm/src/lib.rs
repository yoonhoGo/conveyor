//! Excel WASM Plugin for Conveyor
//!
//! This plugin provides support for reading and writing Excel files (.xlsx, .xls).
//! Supports source (read) and sink (write) operations.

use std::collections::HashMap;

// Generate WIT bindings
wit_bindgen::generate!({
    world: "plugin",
    path: "../../conveyor-wasm-plugin-api/wit",
});

// Plugin API version
const PLUGIN_API_VERSION: u32 = 1;

// Helper functions
fn get_config_value<'a>(config: &'a [(String, String)], key: &str) -> Option<&'a str> {
    config
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn data_format_from_json<T: serde::Serialize>(records: &T) -> Result<DataFormat, PluginError> {
    match serde_json::to_vec(records) {
        Ok(bytes) => Ok(DataFormat::JsonRecords(bytes)),
        Err(e) => Err(PluginError::SerializationError(format!(
            "Failed to serialize JSON: {}",
            e
        ))),
    }
}

fn data_format_to_bytes(data: &DataFormat) -> &[u8] {
    match data {
        DataFormat::ArrowIpc(b) => b,
        DataFormat::JsonRecords(b) => b,
        DataFormat::Raw(b) => b,
    }
}

/// Excel plugin structure
struct ExcelPlugin;

impl Guest for ExcelPlugin {
    /// Get plugin metadata
    fn get_metadata() -> PluginMetadata {
        PluginMetadata {
            name: "excel-wasm".to_string(),
            version: "0.1.0".to_string(),
            description: "Excel WASM plugin for reading and writing .xlsx and .xls files"
                .to_string(),
            api_version: PLUGIN_API_VERSION,
        }
    }

    /// Get list of stages this plugin provides
    fn get_capabilities() -> Vec<StageCapability> {
        vec![
            StageCapability {
                name: "excel.read".to_string(),
                stage_type: StageType::Source,
                description: "Read Excel files (.xlsx, .xls) into data records".to_string(),
            },
            StageCapability {
                name: "excel.write".to_string(),
                stage_type: StageType::Sink,
                description: "Write data records to Excel files (.xlsx)".to_string(),
            },
        ]
    }

    /// Execute a stage
    fn execute(stage_name: String, context: ExecutionContext) -> Result<DataFormat, PluginError> {
        match stage_name.as_str() {
            "excel.read" => execute_read(&context.config),
            "excel.write" => execute_write(&context.inputs, &context.config),
            _ => Err(PluginError::ConfigError(format!(
                "Unknown stage: {}. Excel plugin provides 'excel.read' and 'excel.write'",
                stage_name
            ))),
        }
    }

    /// Validate configuration for a stage
    fn validate_config(
        stage_name: String,
        config: Vec<(String, String)>,
    ) -> Result<(), PluginError> {
        match stage_name.as_str() {
            "excel.read" => {
                // Validate read configuration
                if get_config_value(&config, "path").is_none() {
                    return Err(PluginError::ConfigError(
                        "Missing required 'path' configuration for excel.read".to_string(),
                    ));
                }
                Ok(())
            }
            "excel.write" => {
                // Validate write configuration
                if get_config_value(&config, "path").is_none() {
                    return Err(PluginError::ConfigError(
                        "Missing required 'path' configuration for excel.write".to_string(),
                    ));
                }
                Ok(())
            }
            _ => Err(PluginError::ConfigError(format!(
                "Unknown stage: {}",
                stage_name
            ))),
        }
    }
}

/// Execute excel.read (source operation)
fn execute_read(config: &[(String, String)]) -> Result<DataFormat, PluginError> {
    // Get file path
    let path = get_config_value(config, "path").ok_or_else(|| {
        PluginError::ConfigError("Missing required 'path' configuration".to_string())
    })?;

    // Get optional sheet name or index (default to first sheet)
    let sheet = get_config_value(config, "sheet");

    // Get optional header row indicator (default: true)
    let has_headers = get_config_value(config, "has_headers")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    // Read Excel file
    let records = read_excel_file(path, sheet, has_headers)?;

    // Convert to JSON format
    data_format_from_json(&records)
}

/// Execute excel.write (sink operation)
fn execute_write(
    inputs: &[(String, DataFormat)],
    config: &[(String, String)],
) -> Result<DataFormat, PluginError> {
    // Get the first input
    let input_data = inputs.first().ok_or_else(|| {
        PluginError::RuntimeError("excel.write requires at least one input".to_string())
    })?;

    // Get file path
    let path = get_config_value(config, "path").ok_or_else(|| {
        PluginError::ConfigError("Missing required 'path' configuration".to_string())
    })?;

    // Get optional sheet name (default: "Sheet1")
    let sheet_name = get_config_value(config, "sheet").unwrap_or("Sheet1");

    // Get optional header writing indicator (default: true)
    let write_headers = get_config_value(config, "write_headers")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    // Parse input data as JSON records
    let bytes = data_format_to_bytes(&input_data.1);
    let records: Vec<HashMap<String, serde_json::Value>> =
        serde_json::from_slice(bytes).map_err(|e| {
            PluginError::SerializationError(format!("Failed to parse input JSON: {}", e))
        })?;

    // Write to Excel file
    write_excel_file(path, sheet_name, &records, write_headers)?;

    // Return empty result (sinks don't produce output)
    Ok(DataFormat::Raw(vec![]))
}

/// Read Excel file and convert to JSON records
fn read_excel_file(
    path: &str,
    sheet: Option<&str>,
    has_headers: bool,
) -> Result<Vec<HashMap<String, serde_json::Value>>, PluginError> {
    use calamine::{open_workbook_auto, Data, DataType, Reader};

    // Open workbook (auto-detects .xls or .xlsx)
    let mut workbook = open_workbook_auto(path).map_err(|e| {
        PluginError::IoError(format!("Failed to open Excel file '{}': {}", path, e))
    })?;

    // Get worksheet
    let range = if let Some(sheet_name) = sheet {
        // Try to get by name first
        if let Ok(r) = workbook.worksheet_range(sheet_name) {
            r
        } else {
            // Try to parse as index
            if let Ok(idx) = sheet_name.parse::<usize>() {
                let sheet_names = workbook.sheet_names();
                if idx >= sheet_names.len() {
                    return Err(PluginError::ConfigError(format!(
                        "Sheet index {} out of bounds (workbook has {} sheets)",
                        idx,
                        sheet_names.len()
                    )));
                }
                workbook.worksheet_range(&sheet_names[idx]).map_err(|e| {
                    PluginError::IoError(format!("Failed to read sheet at index {}: {}", idx, e))
                })?
            } else {
                return Err(PluginError::ConfigError(format!(
                    "Sheet '{}' not found in workbook",
                    sheet_name
                )));
            }
        }
    } else {
        // Get first sheet
        let sheet_names = workbook.sheet_names();
        if sheet_names.is_empty() {
            return Err(PluginError::IoError(
                "Workbook contains no sheets".to_string(),
            ));
        }
        workbook
            .worksheet_range(&sheet_names[0])
            .map_err(|e| PluginError::IoError(format!("Failed to read first sheet: {}", e)))?
    };

    // Convert to records
    let mut records = Vec::new();
    let mut rows = range.rows();

    // Extract headers if present
    let headers: Vec<String> = if has_headers {
        if let Some(header_row) = rows.next() {
            header_row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    cell.as_string()
                        .map(|s| s.to_owned())
                        .unwrap_or_else(|| format!("column_{}", i))
                })
                .collect()
        } else {
            return Ok(records); // Empty sheet
        }
    } else {
        // Generate default column names
        if let Some(first_row) = range.rows().next() {
            (0..first_row.len())
                .map(|i| format!("column_{}", i))
                .collect()
        } else {
            return Ok(records); // Empty sheet
        }
    };

    // Convert data rows to records
    for row in rows {
        let mut record = HashMap::new();

        for (i, cell) in row.iter().enumerate() {
            let key = headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("column_{}", i));

            let value = match cell {
                Data::Int(n) => serde_json::Value::Number((*n).into()),
                Data::Float(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                Data::String(s) => serde_json::Value::String(s.clone()),
                Data::Bool(b) => serde_json::Value::Bool(*b),
                Data::DateTime(dt) => serde_json::Value::String(dt.to_string()),
                Data::DateTimeIso(dt) => serde_json::Value::String(dt.clone()),
                Data::DurationIso(d) => serde_json::Value::String(d.clone()),
                Data::Error(e) => serde_json::Value::String(format!("ERROR: {:?}", e)),
                Data::Empty => serde_json::Value::Null,
            };

            record.insert(key, value);
        }

        records.push(record);
    }

    Ok(records)
}

/// Write JSON records to Excel file
fn write_excel_file(
    path: &str,
    sheet_name: &str,
    records: &[HashMap<String, serde_json::Value>],
    write_headers: bool,
) -> Result<(), PluginError> {
    use rust_xlsxwriter::Workbook;

    // Create workbook
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(sheet_name).map_err(|e| {
        PluginError::RuntimeError(format!("Failed to set sheet name '{}': {}", sheet_name, e))
    })?;

    if records.is_empty() {
        // Save empty workbook
        workbook.save(path).map_err(|e| {
            PluginError::IoError(format!("Failed to save Excel file '{}': {}", path, e))
        })?;
        return Ok(());
    }

    // Extract column names (preserve order from first record)
    let mut columns: Vec<String> = records[0].keys().cloned().collect();
    columns.sort(); // Sort for consistent output

    let mut row_idx = 0u32;

    // Write headers if requested
    if write_headers {
        for (col_idx, column) in columns.iter().enumerate() {
            worksheet
                .write_string(row_idx, col_idx as u16, column)
                .map_err(|e| PluginError::RuntimeError(format!("Failed to write header: {}", e)))?;
        }
        row_idx += 1;
    }

    // Write data rows
    for record in records {
        for (col_idx, column) in columns.iter().enumerate() {
            if let Some(value) = record.get(column) {
                write_cell_value(worksheet, row_idx, col_idx as u16, value)?;
            }
        }
        row_idx += 1;
    }

    // Save workbook
    workbook.save(path).map_err(|e| {
        PluginError::IoError(format!("Failed to save Excel file '{}': {}", path, e))
    })?;

    Ok(())
}

/// Write a JSON value to an Excel cell
fn write_cell_value(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: &serde_json::Value,
) -> Result<(), PluginError> {
    match value {
        serde_json::Value::Null => {
            // Write empty string for null
            worksheet
                .write_string(row, col, "")
                .map_err(|e| PluginError::RuntimeError(format!("Failed to write cell: {}", e)))?;
        }
        serde_json::Value::Bool(b) => {
            worksheet
                .write_boolean(row, col, *b)
                .map_err(|e| PluginError::RuntimeError(format!("Failed to write cell: {}", e)))?;
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                worksheet.write_number(row, col, i as f64).map_err(|e| {
                    PluginError::RuntimeError(format!("Failed to write cell: {}", e))
                })?;
            } else if let Some(f) = n.as_f64() {
                worksheet.write_number(row, col, f).map_err(|e| {
                    PluginError::RuntimeError(format!("Failed to write cell: {}", e))
                })?;
            }
        }
        serde_json::Value::String(s) => {
            worksheet
                .write_string(row, col, s)
                .map_err(|e| PluginError::RuntimeError(format!("Failed to write cell: {}", e)))?;
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            // For complex types, write JSON string
            let json_str = serde_json::to_string(value).map_err(|e| {
                PluginError::SerializationError(format!("Failed to serialize complex value: {}", e))
            })?;
            worksheet
                .write_string(row, col, &json_str)
                .map_err(|e| PluginError::RuntimeError(format!("Failed to write cell: {}", e)))?;
        }
    }

    Ok(())
}

// Export the plugin implementation
export!(ExcelPlugin);
