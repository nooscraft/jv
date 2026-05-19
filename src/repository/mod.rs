//! Repository access layer for Maven-compatible repositories (Maven Central, custom, etc.).
//!
//! Responsibilities:
//! - HTTP client configuration with connection pooling and timeouts
//! - Fetching `maven-metadata.xml`
//! - Fetching `pom.xml` files for specific GAV coordinates
//! - Supporting multiple repositories with fallback

pub mod client;

pub use client::{MavenMetadata, Repository, RepositoryClient};
