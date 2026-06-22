//! Parallel artifact downloading with progress reporting.
//!
//! This module is one of the primary sources of the "10-100x" performance
//! advantage: instead of downloading dependencies one-by-one (like classic Maven),
//! we fetch many artifacts concurrently while respecting a high but safe
//! concurrency limit and using the shared global cache.

pub mod parallel;

pub use parallel::{DownloadResult, ParallelDownloader};
