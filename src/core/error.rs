use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConveyorError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Data source error: {0}")]
    DataSourceError(String),

    #[error("Transform error: {0}")]
    TransformError(String),

    #[error("Sink error: {0}")]
    SinkError(String),

    #[error("Pipeline execution error: {0}")]
    PipelineError(String),

    #[error("Module not found: {0}")]
    ModuleNotFound(String),

    #[error("Invalid data format: {0}")]
    InvalidDataFormat(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Polars error: {0}")]
    PolarsError(#[from] polars::error::PolarsError),

    #[error("Plugin error: {0}")]
    PluginError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ConveyorError {
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::ConfigError(msg.into())
    }

    pub fn data_source<S: Into<String>>(msg: S) -> Self {
        Self::DataSourceError(msg.into())
    }

    pub fn transform<S: Into<String>>(msg: S) -> Self {
        Self::TransformError(msg.into())
    }

    pub fn sink<S: Into<String>>(msg: S) -> Self {
        Self::SinkError(msg.into())
    }

    pub fn pipeline<S: Into<String>>(msg: S) -> Self {
        Self::PipelineError(msg.into())
    }

    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::ValidationError(msg.into())
    }
}

pub type ConveyorResult<T> = Result<T, ConveyorError>;