//! jv - Fast Java dependency resolver (library crate)
//!
//! The library provides the core resolution engine that the CLI consumes.

pub mod cache;
pub mod cli;
pub mod download;
pub mod error;
pub mod lockfile;
pub mod models;
pub mod parser;
pub mod repository;
pub mod resolver;

// Re-export the most commonly used types for convenience
pub use models::{
    Artifact, Dependency, Exclusion, MavenCoordinate, ResolvedDependency, Scope, Version,
    VersionRange,
};
