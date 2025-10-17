use anyhow::Result;
use clap::Subcommand;

use crate::core::plugin_manager::PluginManager;

#[derive(Debug, Subcommand)]
pub enum PluginCommand {
    /// Install a plugin
    Install {
        /// Plugin name to install
        name: String,
    },

    /// List installed plugins
    List,

    /// Uninstall a plugin
    Uninstall {
        /// Plugin name to uninstall
        name: String,
    },

    /// Show plugin information
    Info {
        /// Plugin name
        name: String,
    },
}

impl PluginCommand {
    pub async fn execute(&self) -> Result<()> {
        match self {
            PluginCommand::Install { name } => install_plugin(name).await,
            PluginCommand::List => list_plugins().await,
            PluginCommand::Uninstall { name } => uninstall_plugin(name),
            PluginCommand::Info { name } => show_plugin_info(name).await,
        }
    }
}

/// Install a plugin
async fn install_plugin(name: &str) -> Result<()> {
    let manager = PluginManager::new()?;

    // Check if already installed
    if manager.is_installed(name) {
        println!("✓ Plugin '{}' is already installed", name);
        return Ok(());
    }

    // Install plugin
    manager.install_plugin(name).await?;

    println!("✓ Plugin '{}' installed successfully", name);
    println!("  Location: {}", manager.plugins_dir().display());

    Ok(())
}

/// List installed plugins
async fn list_plugins() -> Result<()> {
    let manager = PluginManager::new()?;

    // Get installed plugins
    let installed = manager.list_installed()?;

    // Get registry for available plugins
    let registry = manager.get_registry(false).await?;
    let available = registry.list_plugins();

    println!("\n======================================================================");
    println!("Plugin Manager");
    println!("======================================================================\n");

    // Show installed plugins
    if installed.is_empty() {
        println!("Installed Plugins:");
        println!("----------------------------------------------------------------------");
        println!("  (none)\n");
    } else {
        println!("Installed Plugins:");
        println!("----------------------------------------------------------------------");
        for plugin in &installed {
            println!("  • {} ({})", plugin.name, plugin.version);
            println!("    Path: {}", plugin.path.display());
        }
        println!();
    }

    // Show available plugins
    println!("Available for Install:");
    println!("----------------------------------------------------------------------");

    let installed_names: Vec<_> = installed.iter().map(|p| p.name.as_str()).collect();

    for plugin in available {
        let status = if installed_names.contains(&plugin.name.as_str()) {
            "[installed]"
        } else {
            ""
        };
        println!("  • {} ({}) {}", plugin.name, plugin.version, status);
        println!("    {}", plugin.description);
    }

    println!();

    Ok(())
}

/// Uninstall a plugin
fn uninstall_plugin(name: &str) -> Result<()> {
    let manager = PluginManager::new()?;

    manager.uninstall_plugin(name)?;
    println!("✓ Plugin '{}' uninstalled successfully", name);

    Ok(())
}

/// Show plugin information
async fn show_plugin_info(name: &str) -> Result<()> {
    let manager = PluginManager::new()?;

    // Get registry
    let registry = manager.get_registry(false).await?;

    // Find plugin
    let plugin = registry
        .get_plugin(name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in registry", name))?;

    // Check if installed
    let is_installed = manager.is_installed(name);

    println!("\n======================================================================");
    println!("Plugin: {}", plugin.name);
    println!("======================================================================\n");

    println!("Name:        {}", plugin.name);
    println!("Version:     {}", plugin.version);
    println!("Author:      {}", plugin.author);
    println!("Description: {}", plugin.description);
    println!("Repository:  {}", plugin.repository);
    println!("Installed:   {}", if is_installed { "Yes" } else { "No" });

    if is_installed {
        let filename = format!("libconveyor_plugin_{}.dylib", name);
        let path = manager.plugins_dir().join(filename);
        println!("Location:    {}", path.display());
    }

    // Show download info
    if let Some(download) = plugin.get_download_info() {
        println!("\nDownload:");
        println!("  URL:      {}", download.url);
        if !download.checksum.is_empty() {
            println!("  Checksum: {}", download.checksum);
        }
    }

    println!();

    Ok(())
}
