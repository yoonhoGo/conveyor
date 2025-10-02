//! Integration test for plugin loading

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
    assert_eq!(plugin.version(), "0.1.0");
    assert_eq!(plugin.description(), "Simple test plugin");
}

#[test]
fn test_nonexistent_plugin() {
    let mut loader = PluginLoader::new();

    // Try to load a plugin that doesn't exist
    let result = loader.load_plugin("nonexistent");

    // Should fail
    assert!(result.is_err());
}
