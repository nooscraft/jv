//! jv binary entry point. Thin wrapper around the library.

use anyhow::Result;
use clap::Parser;
use jv::cli::{Cli, Commands};
use jv::lockfile::{write_lock_file, LockFile};
use jv::resolver::resolve_direct;
use std::path::PathBuf;

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
            let file = if args.file == PathBuf::from("pom.xml") {
                // Smart default search
                if PathBuf::from("pom.xml").exists() {
                    PathBuf::from("pom.xml")
                } else if PathBuf::from("build.gradle").exists() {
                    PathBuf::from("build.gradle")
                } else {
                    PathBuf::from("pom.xml")
                }
            } else {
                args.file.clone()
            };

            println!("Resolving dependencies from {} ...", file.display());

            if file.extension().map_or(false, |e| e == "xml") || file.ends_with("pom.xml") {
                let (resolved, downloads) = resolve_direct(&file, &args.repositories).await?;

                println!(
                    "Resolved {} direct dependencies ({} downloaded, {} cached).",
                    resolved.len(),
                    downloads.iter().filter(|d| !d.cached).count(),
                    downloads.iter().filter(|d| d.cached).count()
                );

                if !args.dry_run {
                    let lock = LockFile::from_resolved(&resolved);
                    write_lock_file(&args.output, &lock)?;
                    println!("Lock file written to {}", args.output.display());
                } else {
                    println!("Dry run - lock file not written.");
                }
            } else {
                println!("Gradle files are not fully supported in this build (coming soon).");
            }
        }
        Commands::Verify => {
            println!("Verifying lock file against current project...");
        }
        Commands::Update(args) => {
            println!("Updating dependency: {}", args.dependency);
        }
        Commands::Cache(args) => match args.command {
            jv::cli::CacheCommands::Clean => println!("Cleaning cache..."),
            jv::cli::CacheCommands::Prune => println!("Pruning cache..."),
        },
    }

    Ok(())
}
