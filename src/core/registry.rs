use anyhow::Result;
use std::collections::HashMap;

use crate::core::stage::StageRef;

pub struct ModuleRegistry {
    stages: HashMap<String, StageRef>,
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
        }
    }

    pub async fn with_defaults() -> Result<Self> {
        let mut registry = Self::new();
        registry.register_default_modules().await?;
        Ok(registry)
    }

    pub async fn register_default_modules(&mut self) -> Result<()> {
        // Register function-based modules
        let functions = crate::modules::register_functions();
        for (name, stage) in functions {
            self.stages.insert(name, stage);
        }

        // Note: PipelineStage is registered dynamically in DagPipelineBuilder
        // to avoid circular dependencies with the registry

        tracing::debug!("Registered {} functions", self.stages.len());

        Ok(())
    }

    /// Register a stage (function-style)
    pub fn register_stage(&mut self, name: String, stage: StageRef) {
        self.stages.insert(name, stage);
    }

    /// Register a function-style stage (e.g., "csv.read", "mongodb.find")
    /// This is an alias for register_stage but with clearer semantics
    pub fn register_function(&mut self, function_name: String, stage: StageRef) {
        self.stages.insert(function_name, stage);
    }

    /// Get a stage by name
    pub fn get_stage(&self, name: &str) -> Option<&StageRef> {
        self.stages.get(name)
    }

    /// Get a function-style stage (e.g., "csv.read", "mongodb.find")
    /// This is an alias for get_stage but with clearer semantics
    pub fn get_function(&self, function_name: &str) -> Option<&StageRef> {
        self.stages.get(function_name)
    }

    /// List all registered stages/functions
    pub fn list_stages(&self) -> Vec<String> {
        self.stages.keys().cloned().collect()
    }

    /// List all registered functions (alias for list_stages)
    pub fn list_functions(&self) -> Vec<String> {
        self.list_stages()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_initialization() {
        let registry = ModuleRegistry::new();
        assert_eq!(registry.list_stages().len(), 0);
    }

    #[tokio::test]
    async fn test_registry_with_defaults() {
        let registry = ModuleRegistry::with_defaults().await.unwrap();

        // Check that built-in functions are registered
        assert!(registry.list_stages().len() > 0);

        // Check specific functions
        assert!(registry.get_function("csv.read").is_some());
        assert!(registry.get_function("csv.write").is_some());
        assert!(registry.get_function("json.read").is_some());
        assert!(registry.get_function("json.write").is_some());
        assert!(registry.get_function("filter.apply").is_some());
        assert!(registry.get_function("map.apply").is_some());
    }

    #[tokio::test]
    async fn test_registry_list_functions() {
        let registry = ModuleRegistry::with_defaults().await.unwrap();
        let functions = registry.list_functions();

        assert!(functions.contains(&"csv.read".to_string()));
        assert!(functions.contains(&"json.write".to_string()));
        assert!(functions.contains(&"filter.apply".to_string()));
    }
}
