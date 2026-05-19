//! jv - Fast Java dependency resolver (library crate)
//!
//! The library provides the core resolution engine that the CLI consumes.

pub mod cache;
pub mod cli;
pub mod error;
pub mod models;
pub mod parser;
pub mod repository;

// Re-export the most commonly used types for convenience
pub use models::{
    Artifact, Dependency, Exclusion, MavenCoordinate, ResolvedDependency, Scope, Version,
    VersionRange,
};
