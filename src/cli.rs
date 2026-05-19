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

    /// Print the dependency tree from a lock file (or by resolving on the fly).
    Tree(TreeArgs),
}

#[derive(Args, Debug)]
pub struct ResolveArgs {
    /// Path to a pom.xml, build.gradle, build.gradle.kts, or a directory containing one.
    /// If a directory is given, jv will look for pom.xml, then build.gradle, then build.gradle.kts.
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Output path for the generated lock file.
    #[arg(short, long, default_value = "jv.lock")]
    pub output: PathBuf,

    /// Additional Maven repository URL (can be specified multiple times).
    #[arg(short, long = "repo")]
    pub repositories: Vec<String>,

    /// Perform a dry run without writing the lock file.
    #[arg(long)]
    pub dry_run: bool,

    /// Bypass the local cache and fetch everything fresh from the network.
    /// Useful when developing the resolver or when you suspect stale data.
    #[arg(long)]
    pub no_cache: bool,
}

#[derive(Args, Debug)]
pub struct TreeArgs {
    /// Path to jv.lock (if omitted, will look for one or resolve on the fly).
    #[arg(value_name = "LOCKFILE", default_value = "jv.lock")]
    pub lockfile: PathBuf,

    /// Depth to limit the tree (useful for very large Spring Boot apps).
    #[arg(short, long)]
    pub depth: Option<usize>,
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
    Prune {
        /// Remove entries older than this many days (default: 90).
        #[arg(long, default_value_t = 90)]
        max_age_days: u64,
    },
}
