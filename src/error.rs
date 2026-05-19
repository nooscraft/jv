//! Error types for the jv resolver using `thiserror` for ergonomic error handling.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JvError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error while contacting repository {repo}: {source}")]
    Http {
        repo: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("repository returned status {status} for {url}")]
    HttpStatus { status: u16, url: String },

    #[error("failed to parse POM for {coord}: {reason}")]
    PomParse { coord: String, reason: String },

    #[error("failed to parse version '{version}': {reason}")]
    VersionParse { version: String, reason: String },

    #[error("failed to parse version range '{range}': {reason}")]
    VersionRangeParse { range: String, reason: String },

    #[error("dependency not found in any repository: {coord}")]
    DependencyNotFound { coord: String },

    #[error("unresolvable dependency conflict for {group}:{artifact}: {details}")]
    Conflict { group: String, artifact: String, details: String },

    #[error("cache error at {path:?}: {reason}")]
    Cache { path: PathBuf, reason: String },

    #[error("lock file error: {0}")]
    LockFile(String),

    #[error("Gradle build file parsing is not yet supported: {0}")]
    GradleParse(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, JvError>;
