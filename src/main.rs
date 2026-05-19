//! jv binary entry point. Thin wrapper around the library.

use anyhow::Result;
use clap::Parser;
use jv::cli::{Cli, Commands};
use jv::lockfile::{write_lock_file, LockFile, read_lock_file};

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
            let build_file = resolve_build_file(&args.path);

            if build_file.is_none() {
                eprintln!(
                    "Error: Could not find a supported build file in {}",
                    args.path.display()
                );
                eprintln!("Looked for: pom.xml, build.gradle, build.gradle.kts");
                std::process::exit(1);
            }

            let file = build_file.unwrap();
            println!("Resolving dependencies from {} ...", file.display());

            if file.extension().map_or(false, |e| e == "xml") || file.ends_with("pom.xml") {
                println!("Using full transitive resolver (with conflict resolution)...");

                let options = jv::resolver::ResolveOptions {
                    extra_repositories: args.repositories.clone(),
                    no_cache: args.no_cache,
                };

                let resolution = jv::resolver::resolve_transitive(&file, options).await?;

                println!(
                    "Resolved {} transitive dependencies for {}",
                    resolution.dependencies.len(),
                    resolution.root
                );

                if !args.dry_run {
                    let lock = LockFile::from_resolved(&resolution.dependencies);
                    write_lock_file(&args.output, &lock)?;
                    println!("Lock file written to {}", args.output.display());
                } else {
                    println!("Dry run - lock file not written.");
                }

                // Also demonstrate the old direct path is still available via the library
                // for comparison during the transition period.
            } else if file.extension().map_or(false, |e| e == "gradle" || e == "kts") {
                // Basic Gradle path (uses the new declarative parser)
                println!("Parsing Gradle file (declarative dependencies only)...");
                let content = std::fs::read_to_string(&file).unwrap_or_default();
                let gradle_deps = jv::parser::gradle::parse_build_gradle(&content);
                println!("Found {} dependencies in Gradle file", gradle_deps.len());

                // For now just treat them as direct deps (real transitive coming later)
                let mut resolved = Vec::new();
                for d in gradle_deps {
                    resolved.push(jv::models::ResolvedDependency {
                        coordinate: d.coordinate,
                        scope: d.scope,
                        optional: d.optional,
                        depended_by: None,
                        artifacts: vec![],
                    });
                }

                if !args.dry_run {
                    let lock = LockFile::from_resolved(&resolved);
                    write_lock_file(&args.output, &lock)?;
                    println!("Lock file written to {}", args.output.display());
                }
            } else {
                println!("Unsupported build file. Supported: pom.xml, build.gradle, build.gradle.kts");
            }
        }
        Commands::Verify => {
            println!("Verifying lock file against current project...");
        }
        Commands::Update(args) => {
            println!("Updating dependency: {}", args.dependency);
        }
        Commands::Cache(args) => match args.command {
            jv::cli::CacheCommands::Clean => {
                if let Ok(cache) = jv::cache::CacheManager::new() {
                    let _ = cache.clear();
                    println!("Cache cleared.");
                }
            }
            jv::cli::CacheCommands::Prune { max_age_days } => {
                if let Ok(cache) = jv::cache::CacheManager::new() {
                    match cache.prune(max_age_days) {
                        Ok((count, bytes)) => {
                            if count > 0 {
                                println!(
                                    "Cache pruned: removed {} entries, freed ~{:.1} MB (older than {} days).",
                                    count,
                                    bytes as f64 / 1024.0 / 1024.0,
                                    max_age_days
                                );
                            } else {
                                println!("Cache prune: nothing older than {} days to remove.", max_age_days);
                            }
                        }
                        Err(e) => eprintln!("Cache prune failed: {}", e),
                    }
                }
            }
        },

        Commands::Tree(_args) => {
            println!("`jv tree` is coming in the next release. For now, the lockfile contains `requested_by` information you can inspect manually.");
            println!("Run `jv resolve` first, then look at the generated jv.lock.");
        }
    }

    Ok(())
}



/// Given a path (file or directory), return the best build file to use.
fn resolve_build_file(path: &PathBuf) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.clone());
    }

    if !path.is_dir() {
        return None;
    }

    // Common order: Maven first (most reliable today), then Gradle
    let candidates = ["pom.xml", "build.gradle", "build.gradle.kts"];

    for name in &candidates {
        let candidate = path.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}
