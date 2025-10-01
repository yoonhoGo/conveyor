pub mod cli;
pub mod core;
pub mod modules;
pub mod utils;

// Re-export commonly used types
pub use core::config::PipelineConfig;
pub use core::error::{ConveyorError, ConveyorResult};
pub use core::pipeline::Pipeline;
pub use core::registry::ModuleRegistry;
pub use core::traits::{DataFormat, DataSource, Transform, Sink};
