//! CLI argument definitions for the `jv` Java dependency resolver.
//!
//! Uses `clap` derive API for ergonomic subcommand parsing.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// jv - A fast, Rust-based Java dependency resolver inspired by uv.
///
/// Resolve Maven and Gradle dependencies with 10-100x speedups.
#[derive(Parser, Debug)]
#[command(name = "jv", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Resolve dependencies from a Maven or Gradle build file and generate a lock file.
    Resolve(ResolveArgs),

    /// Verify that the current lock file is up-to-date with the project.
    Verify,

    /// Update a specific dependency to a newer version.
    Update(UpdateArgs),

    /// Manage the local artifact cache.
    Cache(CacheArgs),
}

#[derive(Args, Debug)]
pub struct ResolveArgs {
    /// Path to pom.xml, build.gradle, or build.gradle.kts.
    /// If omitted, jv will search the current directory.
    #[arg(value_name = "FILE", default_value = "pom.xml")]
    pub file: PathBuf,

    /// Output path for the generated lock file.
    #[arg(short, long, default_value = "jv.lock")]
    pub output: PathBuf,

    /// Additional Maven repository URL (can be specified multiple times).
    #[arg(short, long = "repo")]
    pub repositories: Vec<String>,

    /// Perform a dry run without writing the lock file.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Dependency to update in group:artifact format or group:artifact:version.
    pub dependency: String,

    /// Allow major version upgrades.
    #[arg(long)]
    pub major: bool,
}

#[derive(Args, Debug)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheCommands,
}

#[derive(Subcommand, Debug)]
pub enum CacheCommands {
    /// Remove all cached artifacts and metadata.
    Clean,

    /// Remove unused or stale entries from the cache.
    Prune,
}
