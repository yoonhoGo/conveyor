pub mod cli;
pub mod core;
pub mod modules;
pub mod plugin_loader;
pub mod utils;
pub mod wasm_plugin_loader;

// Re-export commonly used types
pub use core::config::PipelineConfig;
pub use core::error::{ConveyorError, ConveyorResult};
pub use core::pipeline::Pipeline;
pub use core::registry::ModuleRegistry;
pub use core::traits::{DataFormat, DataSource, Sink, Transform};
pub use plugin_loader::PluginLoader;
pub use wasm_plugin_loader::WasmPluginLoader;
