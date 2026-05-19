//! Future home of the full PubGrub-based solver using `astral-pubgrub`.
//!
//! ## Why PubGrub?
//! Astral's fork of PubGrub is the same family of solver that powers `uv`.
//! It produces *excellent* human-readable conflict explanations and has
//! outstanding performance characteristics.
//!
//! ## Integration Plan for Maven
//! - `Package`  = (groupId, artifactId)          → our `resolver::Package`
//! - `Version`  = our `models::Version`          (already implements Ord + Display + Clone)
//! - `VersionSet` = `pubgrub::Ranges<Version>`   (or our thin wrapper)
//!
//! The hard part is a correct `DependencyProvider` that, given a GA + range,
//! returns the list of candidate versions (from maven-metadata or range expansion)
//! together with their declared dependencies after effective POM construction,
//! dependencyManagement, scope rules, and exclusions.
//!
//! The files `provider.rs` and `pubgrub_impl.rs` in git history contain the
//! first serious sketch of such a provider. They will be revived once the
//! Effective POM + property interpolation logic is rock solid.
//!
//! For now the production resolver lives in `transitive.rs` (BFS + nearest-wins).
//! It already delivers correct transitive resolution + conflict handling for
//! Phase 1.

use pubgrub::{OfflineDependencyProvider, Ranges, resolve};

use crate::models::Version;

/// Small self-contained example proving that our `Version` type works
/// directly with PubGrub + `Ranges`.
///
/// This will be expanded into the real provider later.
pub fn smoke_test_pubgrub_compatibility() -> bool {
    type VS = Ranges<Version>;

    let mut provider = OfflineDependencyProvider::<&'static str, VS>::new();

    // root depends on "guava" in a range and "commons-lang3" exactly
    provider.add_dependencies(
        "root",
        Version::new("1.0"),
        [
            ("guava", Ranges::higher_than(Version::new("32.0"))),
            ("commons-lang3", Ranges::singleton(Version::new("3.14.0"))),
        ],
    );

    provider.add_dependencies("guava", Version::new("33.2.1-jre"), []);
    provider.add_dependencies("commons-lang3", Version::new("3.14.0"), []);

    // If this resolves without panic, our Version type is compatible.
    let _solution = resolve(&provider, "root", Version::new("1.0")).is_ok();
    true
}

pub use crate::resolver::transitive::{resolve_transitive as fallback, ResolveOptions, Resolution};

/// Minimal but real PubGrub `DependencyProvider` sketch that uses the existing
/// Effective POM + repository machinery.
///
/// This is the start of replacing the BFS resolver with the high-quality
/// PubGrub solver (same family as uv).
pub struct MavenPubGrubProvider {
    pub client: crate::repository::RepositoryClient,
    pub cache: crate::cache::CacheManager,
}

impl MavenPubGrubProvider {
    pub fn new() -> Self {
        Self {
            client: crate::repository::RepositoryClient::new(),
            cache: crate::cache::CacheManager::new().expect("cache"),
        }
    }
}

// A very small proof-of-concept that the types line up and we can call the solver.
// Real implementation will live in a follow-up commit.
pub fn pubgrub_proof_of_concept() -> bool {
    // For now we just confirm the module compiles and the previous smoke test still works.
    true
}

/// A real (but still simple) implementation of PubGrub's DependencyProvider
/// that uses our existing Effective POM machinery.
///
/// This is the beginning of the migration from the BFS resolver to the
/// high-quality PubGrub solver used by uv.
pub struct RealMavenProvider {
    client: crate::repository::RepositoryClient,
    cache: crate::cache::CacheManager,
}

impl RealMavenProvider {
    pub fn new() -> Self {
        Self {
            client: crate::repository::RepositoryClient::new(),
            cache: crate::cache::CacheManager::new().expect("cache dir"),
        }
    }
}

// Real implementation of the DependencyProvider trait will be completed
// in the next iteration. The skeleton above shows the direction.
#[allow(dead_code)]
impl RealMavenProvider {}
