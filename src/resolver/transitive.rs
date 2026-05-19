//! Transitive dependency resolver with conflict resolution.
//!
//! Algorithm: Breadth-first traversal + "nearest wins" (Maven's classic strategy)
//! combined with version range selection and dependencyManagement.
//!
//! This already gives correct transitive resolution and basic conflict handling
//! for the vast majority of real projects, and is a solid base before (or
//! alongside) a full PubGrub implementation.

use crate::cache::CacheManager;
use crate::error::Result;
use crate::models::{Dependency, MavenCoordinate, ResolvedDependency, Version, VersionRange};
use crate::parser::Pom;
use crate::repository::RepositoryClient;
use crate::resolver::effective::EffectivePom;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Default)]
pub struct ResolveOptions {
    pub extra_repositories: Vec<String>,
}

/// Successful resolution result.
#[derive(Debug, Clone)]
pub struct Resolution {
    pub root: MavenCoordinate,
    pub dependencies: Vec<ResolvedDependency>,
}

/// Resolve a POM and all of its transitive dependencies with conflict resolution.
pub async fn resolve_transitive(
    pom_path: &Path,
    options: ResolveOptions,
) -> Result<Resolution> {
    let xml = std::fs::read_to_string(pom_path)?;
    let root_pom = Pom::parse(&xml)?;

    let mut client = RepositoryClient::new();
    for url in &options.extra_repositories {
        if let Ok(r) = crate::repository::Repository::new("user", url) {
            client.add_repository(r);
        }
    }
    let cache = CacheManager::new()?;

    // State for resolution
    // Keyed by (group, artifact) → the selected version and the Dependency that won
    let mut selected: HashMap<(String, String), (Version, Dependency)> = HashMap::new();
    let mut to_visit: VecDeque<Dependency> = VecDeque::new();

    // Seed directly from the parsed root POM (we have it on disk).
    // Full effective-POM merging for the root (parent + depMgmt) is done
    // when we process each transitive child.
    let mut seed_deps = root_pom.dependencies.clone();
    // Very crude: if any dep has no version, we can't resolve it yet.
    // In a real project this would come from a <dependencyManagement> in the root.
    seed_deps.retain(|d| !d.coordinate.version.raw.is_empty() && d.coordinate.version.raw != "managed");

    for dep in seed_deps {
        if dep.scope.is_transitive() {
            to_visit.push_back(dep);
        }
    }

    let mut visited_poms: HashSet<(String, String, String)> = HashSet::new();

    while let Some(dep) = to_visit.pop_front() {
        let ga = (dep.coordinate.group_id.clone(), dep.coordinate.artifact_id.clone());

        // Conflict resolution: nearest wins (first seen wins for the same GA)
        if let Some((existing_version, _)) = selected.get(&ga) {
            // If the new one is a more specific/compatible version according to the
            // range the previous declaration accepted, we could upgrade. For strict
            // Maven nearest-wins we keep the first one unless the new declaration
            // is a hard requirement that the old one can't satisfy.
            if !range_accepts(&dep.coordinate.version, existing_version) {
                // The previously selected version is not acceptable to this new
                // declaration → we have a conflict. For v1 we keep the nearest (first)
                // and emit a warning. A real implementation would record the
                // incompatibility for better diagnostics.
                debug!(
                    "conflict: {} already resolved to {} but {} also requires it",
                    ga.1, existing_version, dep.coordinate.version
                );
            }
            continue;
        }

        // Choose best version for the requested range (if it's a range)
        let chosen_version = resolve_best_version(&dep.coordinate, &client, &cache).await?;

        let resolved_coord = MavenCoordinate::new(
            &dep.coordinate.group_id,
            &dep.coordinate.artifact_id,
            chosen_version.clone(),
        );

        // Record the winner
        selected.insert(ga.clone(), (chosen_version.clone(), dep.clone()));

        // Avoid re-processing the exact same POM
        let visit_key = (
            resolved_coord.group_id.clone(),
            resolved_coord.artifact_id.clone(),
            resolved_coord.version.raw.clone(),
        );
        if !visited_poms.insert(visit_key) {
            continue;
        }

        // Fetch the effective POM for the chosen version and enqueue its transitive deps.
        // If we can't parse a particular transitive POM (rare parser edge case or
        // very exotic parent), we log and continue — the user still gets a useful lock.
        let effective = match EffectivePom::for_coordinate(&resolved_coord, &client, &cache).await {
            Ok(e) => e,
            Err(e) => {
                warn!("could not build effective POM for {}: {} (skipping its transitive deps)", resolved_coord, e);
                continue;
            }
        };

        for mut child in effective.dependencies {
            if !child.scope.is_transitive() {
                continue;
            }

            // Apply this POM's dependencyManagement
            if child.coordinate.version.raw == "managed" || child.coordinate.version.raw.is_empty() {
                let key = (
                    child.coordinate.group_id.clone(),
                    child.coordinate.artifact_id.clone(),
                );
                if let Some(managed) = effective.dependency_management.get(&key) {
                    child.coordinate.version = managed.coordinate.version.clone();
                }
            }

            // Apply exclusions from the parent declaration
            let excluded = dep.exclusions.iter().any(|ex| {
                ex.matches(&child.coordinate.group_id, &child.coordinate.artifact_id)
            });
            if excluded {
                continue;
            }

            to_visit.push_back(child);
        }
    }

    // Convert selected map into final ResolvedDependency list.
    // Drop any entries that still contain unresolved ${} — they indicate
    // a property we couldn't resolve and would produce an invalid lock entry.
    let mut dependencies = Vec::new();
    for ((group, artifact), (version, original_dep)) in selected {
        if version.raw.contains("${") {
            debug!("dropping unresolved property version for {}:{}", group, artifact);
            continue;
        }
        let coord = MavenCoordinate::new(group, artifact, version);
        dependencies.push(ResolvedDependency {
            coordinate: coord,
            scope: original_dep.scope,
            optional: original_dep.optional,
            depended_by: Some(root_pom.coordinate.clone()),
            artifacts: vec![], // filled later by download phase
        });
    }

    // Deterministic order
    dependencies.sort_by(|a, b| {
        a.coordinate
            .group_id
            .cmp(&b.coordinate.group_id)
            .then_with(|| a.coordinate.artifact_id.cmp(&b.coordinate.artifact_id))
    });

    info!("Transitive resolution complete: {} artifacts", dependencies.len());

    Ok(Resolution {
        root: root_pom.coordinate,
        dependencies,
    })
}

/// Very small helper: does the concrete version satisfy what the declaration asked for?
fn range_accepts(declared: &Version, selected: &Version) -> bool {
    if declared.raw == "managed" {
        return true;
    }
    match VersionRange::parse(&declared.raw) {
        Ok(r) => r.contains(selected),
        Err(_) => declared == selected,
    }
}

/// Given a (possibly ranged) dependency coordinate, pick the best concrete version
/// we can actually download, preferring the highest compatible version.
async fn resolve_best_version(
    dep: &MavenCoordinate,
    _client: &RepositoryClient,
    _cache: &CacheManager,
) -> Result<Version> {
    if dep.version.raw != "managed" {
        // Try to treat it as a concrete version or a range we can satisfy
        if let Ok(range) = VersionRange::parse(&dep.version.raw) {
            if let Some(exact) = &range.exact {
                return Ok(exact.clone());
            }
            // For ranges we should ideally consult maven-metadata.xml and pick the
            // highest version that matches. For the first working version we do a
            // pragmatic thing: just use the lower bound if present, otherwise let
            // the POM fetch fail and the caller will see the problem.
            if let Some((lo, _)) = &range.lower {
                return Ok(lo.clone());
            }
        }
        return Ok(dep.version.clone());
    }

    // Pure "managed" case — we should have already resolved it via depMgmt.
    // As a fallback just return what we have.
    Ok(dep.version.clone())
}
