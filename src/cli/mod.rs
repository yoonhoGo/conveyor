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
    let modules = registry.list_all_modules();

    if let Some(filter_type) = module_type {
        if let Some(module_list) = modules.get(&filter_type) {
            println!("\n{} modules:", filter_type.to_uppercase());
            println!("{}", "-".repeat(40));
            for module in module_list {
                println!("  • {}", module);
            }
        } else {
            println!("Unknown module type: {}", filter_type);
            println!("Available types: sources, transforms, sinks");
        }
    } else {
        println!("\nAvailable Modules");
        println!("{}", "=".repeat(40));

        for (category, module_list) in modules {
            println!("\n{}:", category.to_uppercase());
            println!("{}", "-".repeat(40));
            for module in module_list {
                println!("  • {}", module);
            }
        }
    }

    println!();
    Ok(())
}
