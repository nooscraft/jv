//! jv - Fast Java dependency resolver (library crate)
//!
//! The library provides the core resolution engine that the CLI consumes.

pub mod models;
pub mod cli;

// Re-export the most commonly used types for convenience
pub use models::{
    Artifact, Dependency, Exclusion, MavenCoordinate, ResolvedDependency, Scope, Version,
    VersionRange,
};
