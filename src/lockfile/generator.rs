//! Lock file (de)serialization.

use crate::error::Result;
use crate::models::ResolvedDependency;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// The top-level structure written to `jv.lock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    pub created_at: String,
    /// All resolved dependencies in a stable order (sorted by GAV)
    pub dependencies: Vec<LockedDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedDependency {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub scope: String,
    pub optional: bool,
}

impl LockFile {
    pub fn from_resolved(resolved: &[ResolvedDependency]) -> Self {
        let mut deps: Vec<_> = resolved
            .iter()
            .map(|r| LockedDependency {
                group: r.coordinate.group_id.clone(),
                artifact: r.coordinate.artifact_id.clone(),
                version: r.coordinate.version.raw.clone(),
                scope: r.scope.to_string(),
                optional: r.optional,
            })
            .collect();

        // Deterministic ordering
        deps.sort_by(|a, b| {
            a.group
                .cmp(&b.group)
                .then_with(|| a.artifact.cmp(&b.artifact))
                .then_with(|| a.version.cmp(&b.version))
        });

        Self {
            version: 1,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_else(|_| "0".to_string()),
            dependencies: deps,
        }
    }
}

/// Write a lock file to disk.
pub fn write_lock_file(path: &Path, lock: &LockFile) -> Result<()> {
    let toml = toml::to_string_pretty(lock).map_err(|e| {
        crate::error::JvError::LockFile(e.to_string())
    })?;
    fs::write(path, toml).map_err(|e| crate::error::JvError::LockFile(e.to_string()))?;
    Ok(())
}

/// Read a lock file from disk.
pub fn read_lock_file(path: &Path) -> Result<LockFile> {
    let content = fs::read_to_string(path).map_err(|e| crate::error::JvError::LockFile(e.to_string()))?;
    let lock: LockFile = toml::from_str(&content).map_err(|e| {
        crate::error::JvError::LockFile(e.to_string())
    })?;
    Ok(lock)
}
