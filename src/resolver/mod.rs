//! Dependency resolution engine (graph construction + conflict resolution).
//!
//! Current status: basic direct-dependency resolution that demonstrates the
//! full pipeline (parse → download in parallel → lock file). Full transitive
//! resolution with nearest-wins conflict handling is the next milestone.

pub mod simple;

pub use simple::resolve_direct;
