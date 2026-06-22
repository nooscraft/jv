//! Deterministic TOML lock file format (jv.lock).
//!
//! The lock file captures the exact set of resolved artifacts so that
//! `jv verify` can guarantee reproducible builds without talking to Maven Central.

pub mod generator;

pub use generator::{read_lock_file, write_lock_file, LockFile};
