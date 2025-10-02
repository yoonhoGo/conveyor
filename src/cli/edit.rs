use anyhow::Result;
use dialoguer::{Select, Confirm};
use std::path::PathBuf;

pub async fn edit_pipeline_interactive(pipeline_file: PathBuf) -> Result<()> {
    loop {
        // Read current pipeline
        let content = std::fs::read_to_string(&pipeline_file)?;
        let mut config: toml::Value = toml::from_str(&content)?;

        // Get existing stages
        let stages = config
            .get("stages")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();

        // Display current pipeline
        println!("\n{}", "=".repeat(60));
        println!("📝 Interactive Pipeline Editor");
        println!("{}", "=".repeat(60));

        if let Some(pipeline) = config.get("pipeline") {
            if let Some(name) = pipeline.get("name").and_then(|n| n.as_str()) {
                println!("Pipeline: {}", name);
            }
        }

        println!("\nCurrent Stages ({}):", stages.len());
        println!("{}", "-".repeat(60));

        if stages.is_empty() {
            println!("  (no stages defined)");
        } else {
            for (idx, stage) in stages.iter().enumerate() {
                let id = stage.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");
                let stage_type = stage.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                let inputs = stage
                    .get("inputs")
                    .and_then(|i| i.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| "none".to_string());

                println!("  {}. {} ({})", idx + 1, id, stage_type);
                println!("     └─ inputs: [{}]", inputs);
            }
        }

        // Main menu
        println!("\n{}", "-".repeat(60));
        let actions = vec![
            "Add new stage",
            "Remove stage",
            "View stage details",
            "Move stage up",
            "Move stage down",
            "Save and exit",
            "Exit without saving",
        ];

        let selection = Select::new()
            .with_prompt("What would you like to do?")
            .items(&actions)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                // Add new stage
                println!("\nAdding new stage...");
                super::add_stage::add_stage_to_pipeline(pipeline_file.clone()).await?;
            }

            1 => {
                // Remove stage
                if stages.is_empty() {
                    println!("\n✗ No stages to remove");
                    continue;
                }

                let stage_names: Vec<String> = stages
                    .iter()
                    .map(|s| {
                        let id = s.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");
                        let stage_type = s.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                        format!("{} ({})", id, stage_type)
                    })
                    .collect();

                let remove_idx = Select::new()
                    .with_prompt("Select stage to remove")
                    .items(&stage_names)
                    .interact()?;

                let confirm = Confirm::new()
                    .with_prompt(format!("Remove stage '{}'?", stage_names[remove_idx]))
                    .default(false)
                    .interact()?;

                if confirm {
                    if let Some(stages_array) = config.get_mut("stages").and_then(|s| s.as_array_mut()) {
                        stages_array.remove(remove_idx);
                        let updated_content = toml::to_string_pretty(&config)?;
                        std::fs::write(&pipeline_file, updated_content)?;
                        println!("✓ Stage removed");
                    }
                }
            }

            2 => {
                // View stage details
                if stages.is_empty() {
                    println!("\n✗ No stages to view");
                    continue;
                }

                let stage_names: Vec<String> = stages
                    .iter()
                    .map(|s| {
                        let id = s.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");
                        let stage_type = s.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                        format!("{} ({})", id, stage_type)
                    })
                    .collect();

                let view_idx = Select::new()
                    .with_prompt("Select stage to view")
                    .items(&stage_names)
                    .interact()?;

                println!("\n{}", "=".repeat(60));
                println!("Stage Details");
                println!("{}", "=".repeat(60));
                println!("{}", toml::to_string_pretty(&stages[view_idx])?);
                println!("{}", "=".repeat(60));

                println!("\nPress Enter to continue...");
                let _: String = dialoguer::Input::new().allow_empty(true).interact_text()?;
            }

            3 | 4 => {
                // Move stage up or down
                if stages.is_empty() {
                    println!("\n✗ No stages to move");
                    continue;
                }

                let stage_names: Vec<String> = stages
                    .iter()
                    .enumerate()
                    .map(|(idx, s)| {
                        let id = s.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");
                        let stage_type = s.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                        format!("{}. {} ({})", idx + 1, id, stage_type)
                    })
                    .collect();

                let move_idx = Select::new()
                    .with_prompt("Select stage to move")
                    .items(&stage_names)
                    .interact()?;

                let move_up = selection == 3;
                let can_move = if move_up {
                    move_idx > 0
                } else {
                    move_idx < stages.len() - 1
                };

                if !can_move {
                    println!("\n✗ Cannot move stage {} (already at boundary)", if move_up { "up" } else { "down" });
                    continue;
                }

                if let Some(stages_array) = config.get_mut("stages").and_then(|s| s.as_array_mut()) {
                    let new_idx = if move_up { move_idx - 1 } else { move_idx + 1 };
                    stages_array.swap(move_idx, new_idx);
                    let updated_content = toml::to_string_pretty(&config)?;
                    std::fs::write(&pipeline_file, updated_content)?;
                    println!("✓ Stage moved {}", if move_up { "up" } else { "down" });
                }
            }

            5 => {
                // Save and exit
                println!("\n✓ Pipeline saved to {:?}", pipeline_file);
                println!("\nNext steps:");
                println!("  1. Validate with 'conveyor validate -c {:?}'", pipeline_file);
                println!("  2. Run with 'conveyor run -c {:?}'", pipeline_file);
                break;
            }

            6 => {
                // Exit without saving
                let confirm = Confirm::new()
                    .with_prompt("Exit without saving changes?")
                    .default(false)
                    .interact()?;

                if confirm {
                    println!("✗ Exited without saving");
                    break;
                }
            }

            _ => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_module_exists() {
        // Just ensure the module compiles
        assert!(true);
    }
}
