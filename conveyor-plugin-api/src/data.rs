//! FFI-safe data format types for Conveyor plugins
//!
//! This module defines FFI-safe equivalents of the core DataFormat enum
//! that can be safely passed across the plugin boundary.

use crate::{RBoxError, RErr, ROk, RResult, RString, RVec, StableAbi};
use serde::Serialize;
use serde_json;

/// FFI-safe data format
///
/// This enum represents different data formats that can be passed between
/// the host and plugins. Since Polars DataFrame and RecordBatch are not
/// FFI-safe, they are serialized to bytes.
#[repr(C)]
#[derive(StableAbi, Debug, Clone)]
pub enum FfiDataFormat {
    /// Arrow IPC serialized DataFrame (Polars)
    ///
    /// This is a Polars DataFrame serialized using Arrow IPC format.
    /// Can be deserialized back to a DataFrame on either side.
    ArrowIpc(RVec<u8>),

    /// JSON serialized record batch
    ///
    /// This is a Vec<HashMap<String, Value>> serialized as JSON.
    /// Flexible format for semi-structured data.
    JsonRecords(RVec<u8>),

    /// Raw bytes
    ///
    /// Unstructured binary data
    Raw(RVec<u8>),
}

impl FfiDataFormat {
    /// Create from Arrow IPC bytes
    pub fn from_arrow_ipc(bytes: Vec<u8>) -> Self {
        Self::ArrowIpc(RVec::from(bytes))
    }

    /// Create from JSON records
    pub fn from_json_records<T: Serialize>(records: &T) -> RResult<Self, RBoxError> {
        match serde_json::to_vec(records) {
            Ok(bytes) => ROk(Self::JsonRecords(RVec::from(bytes))),
            Err(e) => RErr(RBoxError::from_box(Box::new(e))),
        }
    }

    /// Create from raw bytes
    pub fn from_raw(bytes: Vec<u8>) -> Self {
        Self::Raw(RVec::from(bytes))
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::ArrowIpc(bytes) => bytes.as_slice(),
            Self::JsonRecords(bytes) => bytes.as_slice(),
            Self::Raw(bytes) => bytes.as_slice(),
        }
    }

    /// Get the format type as a string
    pub fn format_type(&self) -> RString {
        match self {
            Self::ArrowIpc(_) => RString::from("arrow_ipc"),
            Self::JsonRecords(_) => RString::from("json_records"),
            Self::Raw(_) => RString::from("raw"),
        }
    }

    /// Get the size in bytes
    pub fn size(&self) -> usize {
        self.as_bytes().len()
    }

    /// Convert to JSON records (Vec<HashMap<String, Value>>)
    pub fn to_json_records(
        &self,
    ) -> RResult<Vec<std::collections::HashMap<String, serde_json::Value>>, RBoxError> {
        match self {
            Self::JsonRecords(bytes) => {
                match serde_json::from_slice(bytes.as_slice()) {
                    Ok(records) => ROk(records),
                    Err(e) => RErr(RBoxError::from_box(Box::new(e))),
                }
            }
            Self::Raw(bytes) => {
                // Try to parse raw bytes as JSON
                match serde_json::from_slice(bytes.as_slice()) {
                    Ok(records) => ROk(records),
                    Err(e) => RErr(RBoxError::from_box(Box::new(e))),
                }
            }
            Self::ArrowIpc(_) => {
                RErr(RBoxError::from_fmt(&format_args!(
                    "Cannot convert ArrowIpc to JSON records directly. Use Polars to deserialize first."
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrow_ipc_format() {
        let data = vec![1, 2, 3, 4, 5];
        let format = FfiDataFormat::from_arrow_ipc(data.clone());

        assert_eq!(format.as_bytes(), &data);
        assert_eq!(format.format_type().as_str(), "arrow_ipc");
        assert_eq!(format.size(), 5);
    }

    #[test]
    fn test_json_records_format() {
        use serde::Deserialize;

        #[derive(Serialize, Deserialize)]
        struct Record {
            id: i32,
            name: String,
        }

        let records = vec![
            Record {
                id: 1,
                name: "Alice".to_string(),
            },
            Record {
                id: 2,
                name: "Bob".to_string(),
            },
        ];

        let format = match FfiDataFormat::from_json_records(&records) {
            crate::ROk(f) => f,
            crate::RErr(_) => panic!("Failed to create JSON records format"),
        };

        assert_eq!(format.format_type().as_str(), "json_records");
        assert!(format.size() > 0);

        // Verify we can deserialize back
        let deserialized: Vec<Record> = serde_json::from_slice(format.as_bytes()).unwrap();
        assert_eq!(deserialized.len(), 2);
    }

    #[test]
    fn test_raw_format() {
        let data = b"hello world".to_vec();
        let format = FfiDataFormat::from_raw(data.clone());

        assert_eq!(format.as_bytes(), &data);
        assert_eq!(format.format_type().as_str(), "raw");
        assert_eq!(format.size(), 11);
    }

    #[test]
    fn test_format_matching() {
        let format = FfiDataFormat::from_raw(vec![1, 2, 3]);

        match format {
            FfiDataFormat::Raw(bytes) => {
                assert_eq!(bytes.as_slice(), &[1, 2, 3]);
            }
            _ => panic!("Expected Raw format"),
        }
    }
}
