use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cli;
mod core;
mod modules;
mod utils;

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

    #[command(about = "Generate a sample pipeline configuration")]
    Generate {
        #[arg(short = 'o', long, help = "Output file path")]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = cli.log_level.unwrap_or(Level::INFO);
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
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
            cli::list_modules(module_type)?;
        }

        Commands::Generate { output } => {
            info!("Generating sample configuration");
            cli::generate_sample(output)?;
        }
    }

    Ok(())
}
