//! Local filesystem cache for Maven artifacts and metadata.
//!
//! The cache is global (under `~/.cache/jv` or `$XDG_CACHE_HOME/jv`) so that
//! common dependencies are shared across all projects on the machine — one of
//! the key performance wins over invoking Maven/Gradle per project.

pub mod manager;

pub use manager::CacheManager;
