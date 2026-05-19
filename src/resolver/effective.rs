//! Minimal "effective POM" construction for resolution purposes.
//!
//! Maven's real effective POM is extremely complex (profiles, plugin config,
//! CI activation, etc.). We implement a pragmatic subset that is sufficient
//! for correct dependency resolution in the vast majority of real projects:
//!
//! - Parent chain traversal (with caching)
//! - Property interpolation (basic ${...} support)
//! - dependencyManagement merging (including import scope)
//! - Scope normalization

use crate::cache::CacheManager;
use crate::error::Result;
use crate::models::{Dependency, MavenCoordinate, Scope, Version}; // Scope kept for future import-scope + propagation work
use crate::parser::Pom;
use crate::repository::RepositoryClient;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::debug;

/// Global (for the duration of one `jv resolve` run) POM cache counters.
/// These are only for visibility during testing on large real projects.
pub(crate) static POM_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static POM_CACHE_MISSES: AtomicUsize = AtomicUsize::new(0);

/// The information we need from a POM (and its parents) to resolve dependencies.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EffectivePom {
    pub coordinate: MavenCoordinate,
    pub properties: HashMap<String, String>,
    pub dependencies: Vec<Dependency>,
    /// From <dependencyManagement> (including inherited)
    pub dependency_management: HashMap<(String, String), Dependency>,
}

/// Lightweight result when building an EffectivePom, including cache info.
#[derive(Debug)]
pub struct EffectivePomResult {
    pub pom: EffectivePom,
    /// Whether the root POM for this coordinate came from the local cache.
    pub from_cache: bool,
}

impl EffectivePom {
    /// Build an effective view for a specific coordinate by walking the parent chain.
    pub async fn for_coordinate(
        coord: &MavenCoordinate,
        client: &RepositoryClient,
        cache: &CacheManager,
        no_cache: bool,
    ) -> Result<Self> {
        let mut current = coord.clone();
        let mut chain: Vec<Pom> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Walk up the parent chain (with a safety limit)
        for _ in 0..50 {
            let key = format!("{}:{}", current.group_id, current.artifact_id);
            if !seen.insert(key) {
                break; // cycle guard
            }

            // Try cache first (unless --no-cache)
            let xml = if no_cache {
                debug!("effective POM: --no-cache, fetching {}", current);
                let xml = client.fetch_pom(&current).await?;
                let _ = cache.put_pom(&current, &xml);
                xml
            } else if let Some(xml) = cache.get_pom(&current) {
                POM_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                debug!("effective POM: cache hit for {}", current);
                xml
            } else {
                POM_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
                debug!("effective POM: cache miss, fetching {}", current);
                let xml = client.fetch_pom(&current).await?;
                let _ = cache.put_pom(&current, &xml);
                xml
            };

            let pom = Pom::parse(&xml)?;

            chain.push(pom);

            if let Some(parent) = &chain.last().unwrap().parent {
                current = parent.clone();
            } else {
                break;
            }
        }

        // Now merge from root (last in chain) → child (first)
        // Maven inheritance is child-most wins for most things.
        let mut properties = HashMap::new();
        let mut dep_mgmt = HashMap::new();
        let mut direct_deps = Vec::new();

        for pom in chain.iter().rev() {
            // Properties from parent first, then child overrides
            for (k, v) in &pom.properties {
                properties.insert(k.clone(), v.clone());
            }

            // Add project coordinates as implicit properties
            properties.insert("project.groupId".to_string(), pom.coordinate.group_id.clone());
            properties.insert("project.artifactId".to_string(), pom.coordinate.artifact_id.clone());
            properties.insert("project.version".to_string(), pom.coordinate.version.raw.clone());

            // dependencyManagement (parent first, child wins on conflict)
            for dm in &pom.dependency_management {
                let key = (dm.coordinate.group_id.clone(), dm.coordinate.artifact_id.clone());
                if dm.scope == Scope::Import {
                    let bom_group = &dm.coordinate.group_id;
                    let bom_artifact = &dm.coordinate.artifact_id;
                    let bom_version = &dm.coordinate.version;

                    // Fast path: use previously cached effective data from this BOM
                    if let Some(cached) = cache.get_bom_effective(bom_group, bom_artifact, bom_version) {
                        for (imp_key_str, ver_str) in cached.managed_versions {
                            // Reconstruct a minimal Dependency for dep_mgmt
                            if let Some((g, a)) = imp_key_str.split_once(':') {
                                if let Ok(ver) = Version::parse(&ver_str) {
                                    let mut managed_dep = Dependency::new(g, a, "");
                                    managed_dep.coordinate.version = ver;
                                    let imp_key = (g.to_string(), a.to_string());
                                    dep_mgmt.insert(imp_key, managed_dep);
                                }
                            }
                        }
                        for (k, v) in cached.properties {
                            properties.entry(k).or_insert(v);
                        }
                        continue;
                    }

                    // Slow path: fetch and parse the BOM
                    if let Ok(imported_xml) = client.fetch_pom(&dm.coordinate).await {
                        if let Ok(imported_pom) = Pom::parse(&imported_xml) {
                            // First, collect data we want to cache
                            let mut managed_versions = HashMap::new();
                            for imp_dm in &imported_pom.dependency_management {
                                let k = format!("{}:{}", imp_dm.coordinate.group_id, imp_dm.coordinate.artifact_id);
                                managed_versions.insert(k, imp_dm.coordinate.version.raw.clone());
                            }
                            let bom_properties = imported_pom.properties.clone();

                            // Merge into current effective view
                            for imp_dm in imported_pom.dependency_management {
                                let imp_key = (
                                    imp_dm.coordinate.group_id.clone(),
                                    imp_dm.coordinate.artifact_id.clone(),
                                );
                                dep_mgmt.entry(imp_key).or_insert(imp_dm);
                            }
                            for (k, v) in bom_properties.iter() {
                                properties.entry(k.clone()).or_insert(v.clone());
                            }

                            // Persist for future runs
                            let bom_data = crate::cache::CachedBomData {
                                managed_versions,
                                properties: bom_properties,
                            };
                            let _ = cache.put_bom_effective(bom_group, bom_artifact, bom_version, &bom_data);
                        }
                    }
                    continue;
                }
                dep_mgmt.insert(key, dm.clone());
            }

            // Direct dependencies declared at this level (we'll merge later)
            for d in &pom.dependencies {
                direct_deps.push(d.clone());
            }
        }

        // Interpolate properties into the final direct dependencies and depMgmt
        let mut interpolated_deps: Vec<Dependency> = direct_deps
            .into_iter()
            .map(|mut d| {
                let v = &d.coordinate.version.raw;
                let interpolated = interpolate(&properties, v);
                if let Ok(ver) = Version::parse(&interpolated) {
                    d.coordinate.version = ver;
                }
                d
            })
            .collect();

        // Multi-pass on the direct dependencies too (properties can be defined in terms of other properties)
        for _ in 0..5 {
            let mut changed = false;
            for d in &mut interpolated_deps {
                let before = d.coordinate.version.raw.clone();
                let after = interpolate(&properties, &before);
                if after != before {
                    if let Ok(ver) = Version::parse(&after) {
                        d.coordinate.version = ver;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // Last-resort: many POMs declare a dependency with a property version but
        // actually manage the version in their own <dependencyManagement>.
        // Fill in from dep_mgmt if still unresolved.
        for d in &mut interpolated_deps {
            if d.coordinate.version.raw.contains("${") {
                let key = (
                    d.coordinate.group_id.clone(),
                    d.coordinate.artifact_id.clone(),
                );
                if let Some(managed) = dep_mgmt.get(&key) {
                    d.coordinate.version = managed.coordinate.version.clone();
                }
            }
        }

        // Also interpolate versions inside dependencyManagement
        for dm in dep_mgmt.values_mut() {
            let raw = &dm.coordinate.version.raw;
            let interpolated = interpolate(&properties, raw);
            if let Ok(ver) = Version::parse(&interpolated) {
                dm.coordinate.version = ver;
            }
        }

        // Final multi-pass interpolation — catches cases where a property in one
        // POM is defined using another property from a parent (very common).
        for _ in 0..5 {
            let mut any_change = false;
            for dm in dep_mgmt.values_mut() {
                let before = dm.coordinate.version.raw.clone();
                let after = interpolate(&properties, &before);
                if after != before {
                    if let Ok(ver) = Version::parse(&after) {
                        dm.coordinate.version = ver;
                        any_change = true;
                    }
                }
            }
            if !any_change {
                break;
            }
        }

        Ok(Self {
            coordinate: coord.clone(),
            properties,
            dependencies: interpolated_deps,
            dependency_management: dep_mgmt,
        })
    }
}

/// Very basic ${property} interpolation. Good enough for 95% of real POMs.
pub(crate) fn interpolate(properties: &HashMap<String, String>, value: &str) -> String {
    let mut result = value.to_string();
    let mut changed = true;
    let mut guard = 0;

    while changed && guard < 10 {
        changed = false;
        guard += 1;

        for (key, val) in properties {
            let placeholder = format!("${{{}}}", key);
            if result.contains(&placeholder) {
                result = result.replace(&placeholder, val);
                changed = true;
            }
        }

        // Also handle some well-known implicit ones
        if result.contains("${project.version}") {
            if let Some(v) = properties.get("project.version") {
                result = result.replace("${project.version}", v);
                changed = true;
            }
        }
    }

    result
}
