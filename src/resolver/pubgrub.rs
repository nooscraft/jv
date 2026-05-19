//! Future home of the full PubGrub-based solver using `astral-pubgrub`.
//!
//! See provider.rs and pubgrub_impl.rs in git history for the initial integration work.
//! The long-term goal is to switch the core of `resolve_transitive` to this
//! implementation for superior conflict diagnostics (exactly like uv).

pub use crate::resolver::transitive::{resolve_transitive as fallback, ResolveOptions, Resolution};
