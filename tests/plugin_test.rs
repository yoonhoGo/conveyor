//! Integration test for plugin loading (Version 2 API)

use conveyor::plugin_loader::PluginLoader;

#[test]
fn test_plugin_loading() {
    // Create a plugin loader
    let mut loader = PluginLoader::new();

    // Try to load the test plugin
    let result = loader.load_plugin("test");

    // Should succeed
    assert!(
        result.is_ok(),
        "Failed to load test plugin: {:?}",
        result.err()
    );

    // Verify plugin is loaded
    assert!(loader.is_loaded("test"));

    // Get plugin info
    let plugin = loader.get_plugin("test").expect("Plugin should be loaded");
    assert_eq!(plugin.name(), "test");
    assert_eq!(plugin.version(), "1.0.0");
    assert_eq!(plugin.description(), "Simple test plugin");

    // Check capabilities
    let capabilities = plugin.capabilities();
    assert_eq!(capabilities.len(), 3, "Test plugin should provide 3 stages");

    // Verify stage names
    let stage_names = plugin.stage_names();
    assert!(stage_names.contains(&"test".to_string()));
}

#[test]
fn test_plugin_stage_creation() {
    // Create a plugin loader
    let mut loader = PluginLoader::new();

    // Load the test plugin
    loader
        .load_plugin("test")
        .expect("Failed to load test plugin");

    // Try to create a stage from the plugin
    let result = loader.create_stage("test");
    assert!(
        result.is_ok(),
        "Failed to create test stage: {:?}",
        result.err()
    );
}

#[test]
fn test_nonexistent_plugin() {
    let mut loader = PluginLoader::new();

    // Try to load a plugin that doesn't exist
    let result = loader.load_plugin("nonexistent");

    // Should fail
    assert!(result.is_err());
}

#[test]
fn test_nonexistent_stage() {
    let mut loader = PluginLoader::new();

    // Load the test plugin
    loader
        .load_plugin("test")
        .expect("Failed to load test plugin");

    // Try to create a stage that doesn't exist
    let result = loader.create_stage("nonexistent");
    assert!(result.is_err(), "Should fail to create nonexistent stage");
}
