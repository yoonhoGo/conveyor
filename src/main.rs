use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cli;
mod core;
mod modules;
mod plugin_loader;
mod utils;
mod wasm_plugin_loader;

use crate::core::pipeline::Pipeline;

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
        #[arg(short = 'c', long, help = "Path to the TOML configuration file")]
        config: PathBuf,

        #[arg(long, help = "Validate configuration without running")]
        dry_run: bool,

        #[arg(long, help = "Continue on errors")]
        continue_on_error: bool,
    },

    #[command(about = "Validate a pipeline configuration")]
    Validate {
        #[arg(short = 'c', long, help = "Path to the TOML configuration file")]
        config: PathBuf,
    },

    #[command(about = "List available modules")]
    List {
        #[arg(short = 't', long, help = "Filter by module type")]
        module_type: Option<String>,
    },

    #[command(about = "Stage management commands")]
    Stage {
        #[command(subcommand)]
        command: StageCommands,
    },
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = cli.log_level.unwrap_or(Level::INFO);
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    match cli.command {
        Commands::Run {
            config,
            dry_run,
            continue_on_error,
        } => {
            info!("Loading pipeline configuration from {:?}", config);
            let pipeline = Pipeline::from_file(&config).await?;

            if dry_run {
                info!("Dry run mode - validating configuration");
                pipeline.validate().await?;
                info!("Configuration is valid");
            } else {
                info!("Executing pipeline");
                pipeline.execute(continue_on_error).await?;
                info!("Pipeline execution completed");
            }
        }

        Commands::Validate { config } => {
            info!("Validating pipeline configuration from {:?}", config);
            let pipeline = Pipeline::from_file(&config).await?;
            pipeline.validate().await?;
            println!("✓ Configuration is valid");
        }

        Commands::List { module_type } => {
            info!("Listing available modules");
            cli::list_modules(module_type).await?;
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
        },
    }

    Ok(())
}
