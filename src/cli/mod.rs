use anyhow::Result;

use crate::core::registry::ModuleRegistry;

pub mod add_stage;
pub mod edit;
pub mod scaffold;

pub use add_stage::add_stage_to_pipeline;
pub use edit::edit_pipeline_interactive;
pub use scaffold::scaffold_pipeline;

pub async fn list_modules(module_type: Option<String>) -> Result<()> {
    let registry = ModuleRegistry::with_defaults().await?;
    let all_functions = registry.list_functions();

    // Categorize functions by type based on naming convention
    let mut sources = Vec::new();
    let mut transforms = Vec::new();
    let mut sinks = Vec::new();

    for func in &all_functions {
        if func.ends_with(".read") || func.ends_with(".fetch") || func.ends_with(".find") {
            sources.push(func.clone());
        } else if func.ends_with(".write") || func.ends_with(".insert") {
            sinks.push(func.clone());
        } else {
            transforms.push(func.clone());
        }
    }

    if let Some(filter_type) = module_type {
        let module_list = match filter_type.as_str() {
            "sources" => &sources,
            "transforms" => &transforms,
            "sinks" => &sinks,
            _ => {
                println!("Unknown module type: {}", filter_type);
                println!("Available types: sources, transforms, sinks");
                return Ok(());
            }
        };

        println!("\n{} modules:", filter_type.to_uppercase());
        println!("{}", "-".repeat(40));
        for module in module_list {
            println!("  • {}", module);
        }
    } else {
        println!("\nAvailable Modules");
        println!("{}", "=".repeat(40));

        println!("\nSOURCES:");
        println!("{}", "-".repeat(40));
        for module in &sources {
            println!("  • {}", module);
        }

        println!("\nTRANSFORMS:");
        println!("{}", "-".repeat(40));
        for module in &transforms {
            println!("  • {}", module);
        }

        println!("\nSINKS:");
        println!("{}", "-".repeat(40));
        for module in &sinks {
            println!("  • {}", module);
        }
    }

    println!();
    Ok(())
}
