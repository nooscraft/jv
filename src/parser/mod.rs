//! Build file parsers (Maven POM + basic Gradle support).
//!
//! The POM parser is the most important piece for Phase 1 because the vast
//! majority of Java projects ultimately express their dependency graph via POMs
//! (even Gradle projects often publish effective POMs).

pub mod gradle;
pub mod pom;

pub use pom::Pom;
