use anyhow::Result;
use std::collections::HashMap;

use crate::core::traits::{DataSourceRef, TransformRef, SinkRef};
use crate::modules::{sources, transforms, sinks};

pub struct ModuleRegistry {
    sources: HashMap<String, DataSourceRef>,
    transforms: HashMap<String, TransformRef>,
    sinks: HashMap<String, SinkRef>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            transforms: HashMap::new(),
            sinks: HashMap::new(),
        }
    }

    pub async fn with_defaults() -> Result<Self> {
        let mut registry = Self::new();
        registry.register_default_modules().await?;
        Ok(registry)
    }

    pub async fn register_default_modules(&mut self) -> Result<()> {
        // Register built-in sources
        self.sources.extend(sources::register_sources());

        // Register built-in transforms
        self.transforms.extend(transforms::register_transforms());

        // Register built-in sinks
        self.sinks.extend(sinks::register_sinks());

        tracing::debug!(
            "Registered {} sources, {} transforms, {} sinks",
            self.sources.len(),
            self.transforms.len(),
            self.sinks.len()
        );

        Ok(())
    }

    pub fn register_source(&mut self, name: String, source: DataSourceRef) {
        self.sources.insert(name, source);
    }

    pub fn register_transform(&mut self, name: String, transform: TransformRef) {
        self.transforms.insert(name, transform);
    }

    pub fn register_sink(&mut self, name: String, sink: SinkRef) {
        self.sinks.insert(name, sink);
    }

    pub fn get_source(&self, name: &str) -> Option<&DataSourceRef> {
        self.sources.get(name)
    }

    pub fn get_transform(&self, name: &str) -> Option<&TransformRef> {
        self.transforms.get(name)
    }

    pub fn get_sink(&self, name: &str) -> Option<&SinkRef> {
        self.sinks.get(name)
    }

    pub fn list_sources(&self) -> Vec<String> {
        self.sources.keys().cloned().collect()
    }

    pub fn list_transforms(&self) -> Vec<String> {
        self.transforms.keys().cloned().collect()
    }

    pub fn list_sinks(&self) -> Vec<String> {
        self.sinks.keys().cloned().collect()
    }

    pub fn list_all_modules(&self) -> HashMap<String, Vec<String>> {
        let mut modules = HashMap::new();
        modules.insert("sources".to_string(), self.list_sources());
        modules.insert("transforms".to_string(), self.list_transforms());
        modules.insert("sinks".to_string(), self.list_sinks());
        modules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_initialization() {
        let registry = ModuleRegistry::new();
        assert_eq!(registry.list_sources().len(), 0);
        assert_eq!(registry.list_transforms().len(), 0);
        assert_eq!(registry.list_sinks().len(), 0);
    }

    #[tokio::test]
    async fn test_registry_with_defaults() {
        let registry = ModuleRegistry::with_defaults().await.unwrap();

        // Check that built-in modules are registered
        assert!(registry.list_sources().len() > 0);
        assert!(registry.list_transforms().len() > 0);
        assert!(registry.list_sinks().len() > 0);

        // Check specific modules
        assert!(registry.get_source("csv").is_some());
        assert!(registry.get_source("json").is_some());
        assert!(registry.get_transform("filter").is_some());
        assert!(registry.get_transform("map").is_some());
        assert!(registry.get_sink("csv").is_some());
        assert!(registry.get_sink("json").is_some());
    }

    #[tokio::test]
    async fn test_registry_list_all_modules() {
        let registry = ModuleRegistry::with_defaults().await.unwrap();
        let modules = registry.list_all_modules();

        assert!(modules.contains_key("sources"));
        assert!(modules.contains_key("transforms"));
        assert!(modules.contains_key("sinks"));
    }
}