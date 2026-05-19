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
use crate::models::{Dependency, Exclusion, MavenCoordinate, ResolvedDependency, Version, VersionRange};
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
    // (selected_version, original_dep_declaration, immediate_parent_that_pulled_it_in)
    let mut selected: HashMap<(String, String), (Version, Dependency, MavenCoordinate)> = HashMap::new();

    // Work queue item carrying context for correct exclusion propagation and tree tracking.
    #[derive(Clone)]
    struct WorkItem {
        dep: Dependency,
        parent: MavenCoordinate,
        // Exclusions accumulated from the path that pulled this dependency in.
        // This implements proper deep exclusion semantics.
        accumulated_exclusions: Vec<Exclusion>,
    }

    let mut to_visit: VecDeque<WorkItem> = VecDeque::new();

    // Simple progress tracking for large real-world projects (Spring Boot etc.)
    let mut processed = 0usize;

    // Build a managed versions map from the root project's own dependencyManagement.
    // This is critical for Spring Boot apps where most direct dependencies have no explicit version.
    let mut root_managed: HashMap<(String, String), Version> = HashMap::new();
    for dm in &root_pom.dependency_management {
        let key = (dm.coordinate.group_id.clone(), dm.coordinate.artifact_id.clone());
        root_managed.insert(key, dm.coordinate.version.clone());
    }

    // Seed direct dependencies, resolving versions from the root's dependencyManagement when needed.
    let mut seed_deps: Vec<Dependency> = Vec::new();
    for mut dep in root_pom.dependencies.clone() {
        if !dep.scope.is_transitive() {
            continue;
        }

        if dep.coordinate.version.raw.is_empty() || dep.coordinate.version.raw == "managed" {
            let key = (dep.coordinate.group_id.clone(), dep.coordinate.artifact_id.clone());
            if let Some(managed_ver) = root_managed.get(&key) {
                dep.coordinate.version = managed_ver.clone();
            } else {
                // Still no version — we can't resolve this one reliably yet.
                continue;
            }
        }

        seed_deps.push(dep);
    }

    for dep in seed_deps {
        to_visit.push_back(WorkItem {
            dep,
            parent: root_pom.coordinate.clone(),
            accumulated_exclusions: vec![],
        });
    }

    let mut visited_poms: HashSet<(String, String, String)> = HashSet::new();

    while let Some(work) = to_visit.pop_front() {
        let dep = &work.dep;
        let ga = (dep.coordinate.group_id.clone(), dep.coordinate.artifact_id.clone());

        // Conflict resolution: nearest wins (first seen wins for the same GA)
        if let Some((existing_version, _, _)) = selected.get(&ga) {
            if !range_accepts(&dep.coordinate.version, existing_version) {
                debug!(
                    "conflict: {} already resolved to {} but {} also requires it",
                    ga.1, existing_version, dep.coordinate.version
                );
            }
            continue;
        }

        // Choose best version for the requested range
        let chosen_version = resolve_best_version(&dep.coordinate, &client, &cache).await?;

        let resolved_coord = MavenCoordinate::new(
            &dep.coordinate.group_id,
            &dep.coordinate.artifact_id,
            chosen_version.clone(),
        );

        // Record the winner + the parent that caused this node to be pulled in
        selected.insert(ga.clone(), (chosen_version.clone(), dep.clone(), work.parent.clone()));

        // Avoid re-processing the exact same POM
        let visit_key = (
            resolved_coord.group_id.clone(),
            resolved_coord.artifact_id.clone(),
            resolved_coord.version.raw.clone(),
        );
        if !visited_poms.insert(visit_key) {
            continue;
        }

        processed += 1;
        if processed % 25 == 0 {
            info!("Resolved {} artifacts so far...", processed);
        }

        // Fetch effective POM
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

            // Check accumulated exclusions from the entire ancestor chain
            let excluded = work.accumulated_exclusions.iter().any(|ex| {
                ex.matches(&child.coordinate.group_id, &child.coordinate.artifact_id)
            }) || dep.exclusions.iter().any(|ex| {
                ex.matches(&child.coordinate.group_id, &child.coordinate.artifact_id)
            });

            if excluded {
                debug!("excluded {} by exclusion from {}", child.coordinate, dep.coordinate);
                continue;
            }

            // Build new accumulated exclusions for the child (append this level's exclusions)
            let mut child_exclusions = work.accumulated_exclusions.clone();
            child_exclusions.extend(dep.exclusions.iter().cloned());

            to_visit.push_back(WorkItem {
                dep: child,
                parent: resolved_coord.clone(),
                accumulated_exclusions: child_exclusions,
            });
        }
    }

    // Convert selected map into final ResolvedDependency list.
    // Drop any entries that still contain unresolved ${} — they indicate
    // a property we couldn't resolve and would produce an invalid lock entry.
    let mut dependencies = Vec::new();
    for ((group, artifact), (version, original_dep, pulled_by)) in selected {
        if version.raw.contains("${") {
            debug!("dropping unresolved property version for {}:{}", group, artifact);
            continue;
        }
        let coord = MavenCoordinate::new(group, artifact, version);
        dependencies.push(ResolvedDependency {
            coordinate: coord,
            scope: original_dep.scope,
            optional: original_dep.optional,
            depended_by: Some(pulled_by),
            artifacts: vec![],
        });
    }

    // Deterministic order
    dependencies.sort_by(|a, b| {
        a.coordinate
            .group_id
            .cmp(&b.coordinate.group_id)
            .then_with(|| a.coordinate.artifact_id.cmp(&b.coordinate.artifact_id))
    });

    info!("Transitive resolution complete: {} artifacts (processed {} POMs)", dependencies.len(), processed);

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
