mod cli;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jv=info".parse().unwrap_or_default()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Resolve(args) => {
            println!("Resolving dependencies for: {:?}", args.file);
            // TODO: implement resolution pipeline
            println!("(stub) Resolution complete. Lock file would be written here.");
        }
        Commands::Verify => {
            println!("Verifying lock file against current project...");
        }
        Commands::Update(args) => {
            println!("Updating dependency: {}", args.dependency);
        }
        Commands::Cache(args) => match args.command {
            cli::CacheCommands::Clean => println!("Cleaning cache..."),
            cli::CacheCommands::Prune => println!("Pruning cache..."),
        },
    }

    Ok(())
}
