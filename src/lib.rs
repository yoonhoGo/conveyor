#![allow(dead_code)]

pub mod cli;
pub mod core;
pub mod modules;
pub mod plugin_loader;
pub mod utils;
pub mod wasm_plugin_loader;

// Re-export commonly used types
pub use core::config::DagPipelineConfig;
pub use core::error::{ConveyorError, ConveyorResult};
pub use core::pipeline::DagPipeline;
pub use core::registry::ModuleRegistry;
pub use core::stage::Stage;
pub use core::traits::DataFormat;
pub use plugin_loader::PluginLoader;
pub use wasm_plugin_loader::WasmPluginLoader;
