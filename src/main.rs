#![allow(dead_code)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cli;
mod core;
mod modules;
mod plugin_loader;
mod update;
mod utils;
mod wasm_plugin_loader;

use crate::core::pipeline::DagPipeline;

#[derive(Parser)]
#[command(
    name = "conveyor",
    about = "A TOML-based ETL CLI tool for data pipelines",
    version,
    author
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, help = "Set the log level")]
    log_level: Option<Level>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Run a pipeline from a TOML configuration file")]
    Run {
        #[arg(help = "Path to the TOML configuration file")]
        config: PathBuf,

        #[arg(long, help = "Validate configuration without running")]
        dry_run: bool,
    },

    #[command(about = "Validate a pipeline configuration")]
    Validate {
        #[arg(help = "Path to the TOML configuration file")]
        config: PathBuf,
    },

    #[command(about = "List available modules")]
    List {
        #[arg(short = 't', long, help = "Filter by module type")]
        module_type: Option<String>,
    },

    #[command(about = "Show detailed information about a function")]
    Info {
        #[arg(help = "Function name to get information for")]
        function: String,
    },

    #[command(about = "Build a stage interactively with guided prompts")]
    Build,

    #[command(about = "Stage management commands")]
    Stage {
        #[command(subcommand)]
        command: StageCommands,
    },

    #[command(about = "Update conveyor to the latest version")]
    Update,
}

#[derive(Subcommand)]
enum StageCommands {
    #[command(about = "Create a new DAG-based pipeline template")]
    New {
        #[arg(short = 'o', long, help = "Output file path")]
        output: Option<PathBuf>,

        #[arg(short = 'i', long, help = "Interactive mode with prompts")]
        interactive: bool,
    },

    #[command(about = "Add a new stage to an existing pipeline")]
    Add {
        #[arg(help = "Path to the pipeline TOML file")]
        pipeline: PathBuf,
    },

    #[command(about = "Edit pipeline stages interactively")]
    Edit {
        #[arg(help = "Path to the pipeline TOML file")]
        pipeline: PathBuf,
    },

    #[command(about = "Describe a function in JSON format")]
    Describe {
        #[arg(help = "Function name to describe")]
        function: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = cli.log_level.unwrap_or(Level::INFO);
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Check for updates (once per 24 hours)
    let _ = update::check_for_updates(false).await;

    match cli.command {
        Commands::Run { config, dry_run } => {
            info!("Loading pipeline configuration from {:?}", config);
            let pipeline = DagPipeline::from_file(&config).await?;

            if dry_run {
                info!("Dry run mode - validating configuration");
                pipeline.validate()?;
                info!("Configuration is valid");
            } else {
                info!("Executing pipeline");
                pipeline.execute().await?;
                info!("Pipeline execution completed");
            }
        }

        Commands::Validate { config } => {
            info!("Validating pipeline configuration from {:?}", config);
            let pipeline = DagPipeline::from_file(&config).await?;
            pipeline.validate()?;
            println!("✓ Configuration is valid");
        }

        Commands::List { module_type } => {
            info!("Listing available modules");
            cli::list_modules(module_type).await?;
        }

        Commands::Info { function } => {
            info!("Showing information for function: {}", function);
            cli::show_function_help(&function).await?;
        }

        Commands::Build => {
            info!("Starting interactive stage builder");
            cli::build_stage_interactive().await?;
        }

        Commands::Stage { command } => match command {
            StageCommands::New {
                output,
                interactive,
            } => {
                info!("Creating new DAG pipeline");
                cli::scaffold_pipeline(output.clone(), interactive)?;
            }

            StageCommands::Add { pipeline } => {
                info!("Adding stage to pipeline: {:?}", pipeline);
                cli::add_stage_to_pipeline(pipeline.clone()).await?;
            }

            StageCommands::Edit { pipeline } => {
                info!("Opening interactive pipeline editor: {:?}", pipeline);
                cli::edit_pipeline_interactive(pipeline.clone()).await?;
            }

            StageCommands::Describe { function } => {
                info!("Describing function: {}", function);
                cli::describe_function_json(&function).await?;
            }
        },

        Commands::Update => {
            update::install_update().await?;
        }
    }

    Ok(())
}
