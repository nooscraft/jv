//! Filesystem-backed cache manager with human-readable layout.
//!
//! Layout (under cache root):
//!   metadata/<group>/<artifact>/maven-metadata.xml
//!   poms/<group>/<artifact>/<version>.pom
//!   artifacts/<group>/<artifact>/<version>[-<classifier>].<ext>
//!
//! The layout is intentionally human-inspectable and easy to rsync or clean.

use crate::error::{JvError, Result};
use crate::models::{Artifact, MavenCoordinate};
use dirs::cache_dir;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Clone)]
pub struct CacheManager {
    root: PathBuf,
}

impl CacheManager {
    /// Create a cache manager using the platform-appropriate cache directory.
    pub fn new() -> Result<Self> {
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

        Ok(Self { root })
    }

    /// Create with an explicit root (primarily for tests).
    pub fn with_root(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| JvError::Cache {
            path: root.clone(),
            reason: e.to_string(),
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    // ---------- Metadata ----------

    pub fn metadata_path(&self, group_id: &str, artifact_id: &str) -> PathBuf {
        self.root
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
        self.root
            .join("poms")
            .join(coord.group_id.replace('.', "/"))
            .join(&coord.artifact_id)
            .join(format!("{}.pom", coord.version))
    }

    pub fn get_pom(&self, coord: &MavenCoordinate) -> Option<String> {
        let path = self.pom_path(coord);
        fs::read_to_string(&path).ok().inspect(|_| {
            debug!("cache hit for POM {}", coord);
        })
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
            path,
            reason: e.to_string(),
        })?;
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

        self.root
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
    pub fn clear(&self) -> Result<()> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root).map_err(|e| JvError::Cache {
                path: self.root.clone(),
                reason: e.to_string(),
            })?;
        }
        Ok(())
    }

    /// Best-effort prune of empty directories (can be extended with LRU + size limits later).
    pub fn prune(&self) -> Result<()> {
        // Placeholder for future sophistication (size budgets, last-access tracking, etc.)
        debug!("cache prune requested (no-op in current implementation)");
        Ok(())
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new().expect("failed to initialize default cache manager")
    }
}
