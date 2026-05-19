//! Dependency resolution engine.
//!
//! Current implementation: classic but correct BFS + nearest-wins conflict
//! resolution with proper version range handling and dependencyManagement.
//!
//! Long-term direction (already prepared): full PubGrub solver using
//! `astral-pubgrub` (the same high-quality solver that powers uv). The
//! `pubgrub_impl.rs` and `provider.rs` files contain the start of that work.

mod effective;
pub mod pubgrub; // skeleton for future PubGrub-based solver (astral-pubgrub)

mod transitive;

pub use transitive::{resolve_transitive, ResolveOptions, Resolution};

/// Legacy direct-only resolver (kept for comparison / tests during transition).
pub mod simple;
pub use simple::resolve_direct;
