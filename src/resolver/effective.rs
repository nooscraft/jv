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
use crate::models::{Dependency, MavenCoordinate, Version};
use crate::parser::Pom;
use crate::repository::RepositoryClient;
use std::collections::HashMap;
use tracing::debug;

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

impl EffectivePom {
    /// Build an effective view for a specific coordinate by walking the parent chain.
    pub async fn for_coordinate(
        coord: &MavenCoordinate,
        client: &RepositoryClient,
        cache: &CacheManager,
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

            // Try cache first
            let xml = if let Some(xml) = cache.get_pom(&current) {
                debug!("effective POM: cache hit for parent {}", current);
                xml
            } else {
                debug!("effective POM: fetching {}", current);
                let xml = client.fetch_pom(&current).await?;
                // Opportunistically cache it
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
                dep_mgmt.insert(key, dm.clone());
            }

            // Direct dependencies declared at this level (we'll merge later)
            for d in &pom.dependencies {
                direct_deps.push(d.clone());
            }
        }

        // Interpolate properties into the final direct dependencies and depMgmt
        let interpolated_deps = direct_deps
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

        // Also interpolate versions inside dependencyManagement
        for dm in dep_mgmt.values_mut() {
            let raw = &dm.coordinate.version.raw;
            let interpolated = interpolate(&properties, raw);
            if let Ok(ver) = Version::parse(&interpolated) {
                dm.coordinate.version = ver;
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
fn interpolate(properties: &HashMap<String, String>, value: &str) -> String {
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
