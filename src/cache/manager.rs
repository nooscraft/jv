//! Filesystem-backed cache manager with human-readable layout.
//!
//! Layout (under cache root):
//!   metadata/<group>/<artifact>/maven-metadata.xml
//!   poms/<group>/<artifact>/<version>.pom
//!   artifacts/<group>/<artifact>/<version>[-<classifier>].<ext>
//!
//! The layout is intentionally human-inspectable and easy to rsync or clean.

use crate::error::{JvError, Result};
use crate::models::{Artifact, MavenCoordinate, Version};
use dirs::cache_dir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

#[derive(Clone)]
pub struct CacheManager {
    root: PathBuf,
    /// Optional namespace for this cache instance.
    /// Used to separate caches for different Maven repositories.
    /// Maven Central uses no namespace (empty).
    repo_namespace: Option<String>,
}

/// Cached effective data extracted from an imported BOM (e.g. spring-boot-dependencies).
/// This allows much faster repeated resolutions on Spring Boot / BOM-heavy projects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CachedBomData {
    /// GA -> Version (from the BOM's dependencyManagement)
    pub managed_versions: HashMap<String, String>,
    /// Key properties from the BOM that are commonly used for version interpolation
    pub properties: HashMap<String, String>,
}

impl CacheManager {
    /// Create a cache manager using the platform-appropriate cache directory.
    pub fn new() -> Result<Self> {
        Self::new_with_namespace(None)
    }

    /// Create a cache manager with an optional repository namespace.
    pub fn new_with_namespace(namespace: Option<String>) -> Result<Self> {
        let root = cache_dir()
            .ok_or_else(|| JvError::Cache {
                path: PathBuf::from("~/.cache"),
                reason: "unable to determine user cache directory".to_string(),
            })?
            .join("jv");

        fs::create_dir_all(&root).map_err(|e| JvError::Cache {
            path: root.clone(),
            reason: e.to_string(),
        })?;

        // Pre-create the repos/ directory so namespaced caches have a home
        let _ = fs::create_dir_all(root.join("repos"));

        Ok(Self {
            root,
            repo_namespace: namespace,
        })
    }

    /// Create with an explicit root (primarily for tests).
    pub fn with_root(root: impl Into<PathBuf>) -> Result<Self> {
        Self::with_root_and_namespace(root, None)
    }

    /// Create with explicit root + optional namespace (for tests).
    pub fn with_root_and_namespace(
        root: impl Into<PathBuf>,
        namespace: Option<String>,
    ) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| JvError::Cache {
            path: root.clone(),
            reason: e.to_string(),
        })?;
        Ok(Self {
            root,
            repo_namespace: namespace,
        })
    }

    /// Returns the effective root path, including the repository namespace if present.
    fn effective_root(&self) -> PathBuf {
        match &self.repo_namespace {
            Some(ns) => self.root.join("repos").join(ns),
            None => self.root.clone(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the namespace of this cache instance, if any.
    pub fn namespace(&self) -> Option<&str> {
        self.repo_namespace.as_deref()
    }

    /// Returns basic stats about the cache (best effort).
    pub fn stats(&self) -> (usize, u64) {
        // Very rough implementation: count files and sum sizes under effective root
        let mut file_count = 0usize;
        let mut total_size: u64 = 0;

        fn walk_and_sum(dir: &Path, count: &mut usize, size: &mut u64) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk_and_sum(&path, count, size);
                    } else if let Ok(meta) = entry.metadata() {
                        *count += 1;
                        *size += meta.len();
                    }
                }
            }
        }

        walk_and_sum(&self.effective_root(), &mut file_count, &mut total_size);
        (file_count, total_size)
    }

    /// Create a new CacheManager for a specific repository URL.
    /// All repositories share the global cache root; the coordinator key
    /// (group:artifact:version) is already unique across repos.
    pub fn for_repository(_repo_url: &str) -> Result<Self> {
        Self::new()
    }

    fn is_maven_central(url: &str) -> bool {
        url.contains("repo.maven.apache.org") || url.contains("maven-central.storage-download")
    }

    // ---------- Metadata ----------

    pub fn metadata_path(&self, group_id: &str, artifact_id: &str) -> PathBuf {
        self.effective_root()
            .join("metadata")
            .join(group_id.replace('.', "/"))
            .join(artifact_id)
            .join("maven-metadata.xml")
    }

    pub fn get_metadata(&self, group_id: &str, artifact_id: &str) -> Option<String> {
        let path = self.metadata_path(group_id, artifact_id);
        fs::read_to_string(&path).ok().inspect(|_| {
            debug!("cache hit for metadata {}:{}", group_id, artifact_id);
        })
    }

    pub fn put_metadata(&self, group_id: &str, artifact_id: &str, xml: &str) -> Result<()> {
        let path = self.metadata_path(group_id, artifact_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| JvError::Cache {
                path: parent.to_path_buf(),
                reason: e.to_string(),
            })?;
        }
        fs::write(&path, xml).map_err(|e| JvError::Cache {
            path,
            reason: e.to_string(),
        })?;
        Ok(())
    }

    // ---------- POMs ----------

    pub fn pom_path(&self, coord: &MavenCoordinate) -> PathBuf {
        self.effective_root()
            .join("poms")
            .join(coord.group_id.replace('.', "/"))
            .join(&coord.artifact_id)
            .join(format!("{}.pom", coord.version))
    }

    pub fn get_pom(&self, coord: &MavenCoordinate) -> Option<String> {
        let path = self.pom_path(coord);
        let content = fs::read_to_string(&path).ok()?;

        // Optional integrity check via sidecar hash
        let hash_path = path.with_extension("pom.sha256");
        if hash_path.exists() {
            if let Ok(expected_hash) = fs::read_to_string(&hash_path) {
                let actual = format!("{:x}", Sha256::digest(content.as_bytes()));
                if actual != expected_hash.trim() {
                    debug!(
                        "POM cache integrity check failed for {} — ignoring cached copy",
                        coord
                    );
                    return None;
                }
            }
        }

        debug!("cache hit for POM {}", coord);
        Some(content)
    }

    pub fn put_pom(&self, coord: &MavenCoordinate, xml: &str) -> Result<()> {
        let path = self.pom_path(coord);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| JvError::Cache {
                path: parent.to_path_buf(),
                reason: e.to_string(),
            })?;
        }
        fs::write(&path, xml).map_err(|e| JvError::Cache {
            path: path.clone(),
            reason: e.to_string(),
        })?;

        // Write content hash sidecar for integrity validation on future reads
        let hash = format!("{:x}", Sha256::digest(xml.as_bytes()));
        let hash_path = path.with_extension("pom.sha256");
        let _ = fs::write(hash_path, hash);

        Ok(())
    }

    // ---------- Artifacts (jars, sources, etc.) ----------

    pub fn artifact_path(&self, artifact: &Artifact) -> PathBuf {
        let mut name = format!(
            "{}-{}",
            artifact.coordinate.artifact_id, artifact.coordinate.version
        );
        if let Some(classifier) = &artifact.classifier {
            if !classifier.is_empty() {
                name.push('-');
                name.push_str(classifier);
            }
        }
        name.push('.');
        name.push_str(&artifact.extension);

        self.effective_root()
            .join("artifacts")
            .join(artifact.coordinate.group_id.replace('.', "/"))
            .join(&artifact.coordinate.artifact_id)
            .join(name)
    }

    pub fn get_artifact(&self, artifact: &Artifact) -> Option<PathBuf> {
        let path = self.artifact_path(artifact);
        if path.exists() {
            debug!("cache hit for artifact {}", path.display());
            Some(path)
        } else {
            None
        }
    }

    /// Write artifact bytes to the cache and return the path.
    pub fn put_artifact(&self, artifact: &Artifact, bytes: &[u8]) -> Result<PathBuf> {
        let path = self.artifact_path(artifact);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| JvError::Cache {
                path: parent.to_path_buf(),
                reason: e.to_string(),
            })?;
        }
        fs::write(&path, bytes).map_err(|e| JvError::Cache {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        Ok(path)
    }

    // ---------- Maintenance ----------

    /// Remove the entire cache (dangerous, mostly for tests or `jv cache clean`).
    /// If this instance has a namespace, only that namespace is cleared.
    pub fn clear(&self) -> Result<()> {
        let target = self.effective_root();
        if target.exists() {
            fs::remove_dir_all(&target).map_err(|e| JvError::Cache {
                path: target,
                reason: e.to_string(),
            })?;
        }
        Ok(())
    }

    /// Prune old entries from the cache.
    ///
    /// Removes any file older than `max_age_days`.
    /// Also removes empty directories.
    ///
    /// Returns the number of files removed and the approximate bytes freed.
    pub fn prune(&self, max_age_days: u64) -> Result<(usize, u64)> {
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(max_age_days * 24 * 3600))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let mut removed = 0usize;
        let mut freed_bytes: u64 = 0;

        for dir in ["poms", "artifacts", "metadata"] {
            let dir_path = self.root.join(dir);
            if !dir_path.exists() {
                continue;
            }

            let entries = walk_dir(&dir_path);
            for entry in entries {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let size = metadata.len();
                            if let Err(e) = fs::remove_file(&entry) {
                                debug!("Failed to remove old cache entry {:?}: {}", entry, e);
                            } else {
                                removed += 1;
                                freed_bytes += size;
                            }
                        }
                    }
                }
            }

            // Best-effort: remove empty directories
            let _ = remove_empty_dirs(&dir_path);
        }

        if removed > 0 {
            info!(
                "Cache prune removed {} old entries, freed ~{:.1} MB",
                removed,
                freed_bytes as f64 / 1024.0 / 1024.0
            );
        } else {
            debug!(
                "Cache prune: nothing to remove (all entries younger than {} days)",
                max_age_days
            );
        }

        Ok((removed, freed_bytes))
    }

    // ---------- BOM Effective Data (for import-scoped BOMs) ----------

    fn bom_effective_path(&self, group_id: &str, artifact_id: &str, version: &Version) -> PathBuf {
        self.effective_root()
            .join("effective-boms")
            .join(group_id.replace('.', "/"))
            .join(artifact_id)
            .join(format!("{}.json", version))
    }

    /// Try to load previously extracted effective data for an imported BOM.
    pub fn get_bom_effective(
        &self,
        group_id: &str,
        artifact_id: &str,
        version: &Version,
    ) -> Option<CachedBomData> {
        let path = self.bom_effective_path(group_id, artifact_id, version);
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .inspect(|_| {
                debug!(
                    "cache hit for effective BOM data {}:{}:{}",
                    group_id, artifact_id, version
                );
            })
    }

    /// Store the effective data we extracted from an imported BOM so future
    /// resolutions of the same project (or similar ones) are much faster.
    pub fn put_bom_effective(
        &self,
        group_id: &str,
        artifact_id: &str,
        version: &Version,
        data: &CachedBomData,
    ) -> Result<()> {
        let path = self.bom_effective_path(group_id, artifact_id, version);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| JvError::Cache {
                path: parent.to_path_buf(),
                reason: e.to_string(),
            })?;
        }
        let json = serde_json::to_string_pretty(data).map_err(|e| JvError::Cache {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        fs::write(&path, json).map_err(|e| JvError::Cache {
            path,
            reason: e.to_string(),
        })?;
        Ok(())
    }
}

// --- Helper functions for prune ---

fn walk_dir(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk_dir(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}

fn remove_empty_dirs(dir: &std::path::Path) -> std::io::Result<()> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let _ = remove_empty_dirs(&path);
                // Try to remove if now empty
                let _ = fs::remove_dir(&path);
            }
        }
    }
    Ok(())
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new().expect("failed to initialize default cache manager")
    }
}
