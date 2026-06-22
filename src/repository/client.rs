//! Asynchronous Maven repository client.
//!
//! Designed for high-throughput parallel usage. The client is cheap to clone
//! (internally uses `Arc` for the underlying `reqwest::Client`).

use crate::error::{JvError, Result};
use crate::models::{Artifact, MavenCoordinate, Version};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::{Client, StatusCode, Url};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Information about a single Maven repository.
#[derive(Debug, Clone)]
pub struct Repository {
    /// Unique identifier (e.g. "central", "my-company")
    pub id: String,
    /// Base URL, e.g. "https://repo.maven.apache.org/maven2"
    pub url: Url,
    /// Whether to allow snapshots from this repository
    pub snapshots: bool,
    /// Optional HTTP basic auth
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Repository {
    /// Maven Central (the default).
    pub fn maven_central() -> Self {
        Self {
            id: "central".to_string(),
            url: Url::parse("https://repo.maven.apache.org/maven2/").unwrap(),
            snapshots: false,
            username: None,
            password: None,
        }
    }

    pub fn new(id: impl Into<String>, url: impl AsRef<str>) -> Result<Self> {
        let url = Url::parse(url.as_ref()).map_err(|e| JvError::Other(e.into()))?;
        Ok(Self {
            id: id.into(),
            url,
            snapshots: false,
            username: None,
            password: None,
        })
    }
}

/// Parsed `maven-metadata.xml` content (subset of fields we care about).
#[derive(Debug, Clone, Default)]
pub struct MavenMetadata {
    pub group_id: String,
    pub artifact_id: String,
    pub latest: Option<Version>,
    pub release: Option<Version>,
    pub versions: Vec<Version>,
    pub last_updated: Option<String>,
}

/// High-level asynchronous client for talking to Maven repositories.
#[derive(Clone)]
pub struct RepositoryClient {
    inner: Arc<Inner>,
}

struct Inner {
    client: Client,
    repositories: Vec<Repository>,
}

impl RepositoryClient {
    /// Create a new client with the default Maven Central and sensible timeouts.
    pub fn new() -> Self {
        Self::with_repositories(vec![Repository::maven_central()])
    }

    /// Create a client targeting the provided list of repositories (in priority order).
    pub fn with_repositories(repositories: Vec<Repository>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("jv/0.1 (https://github.com/nooscraft/jv)"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .gzip(true)
            .build()
            .expect("failed to build reqwest client");

        Self {
            inner: Arc::new(Inner {
                client,
                repositories,
            }),
        }
    }

    /// Fetch raw bytes for any artifact (jar, sources, javadoc, etc.).
    /// Tries repositories in order and returns on the first success.
    pub async fn fetch_artifact_bytes(&self, artifact: &Artifact) -> Result<Vec<u8>> {
        let mut remote_path = format!(
            "{}/{}/{}-{}",
            artifact.coordinate.path(),
            artifact.coordinate.version,
            artifact.coordinate.artifact_id,
            artifact.coordinate.version
        );
        if let Some(classifier) = &artifact.classifier {
            if !classifier.is_empty() {
                remote_path.push('-');
                remote_path.push_str(classifier);
            }
        }
        remote_path.push('.');
        remote_path.push_str(&artifact.extension);

        for repo in &self.inner.repositories {
            let url = repo
                .url
                .join(&remote_path)
                .map_err(|e| JvError::Other(e.into()))?;

            debug!("fetching artifact {} from {}", remote_path, repo.id);

            match self.fetch_bytes(&url, &repo.id).await {
                Ok(bytes) => return Ok(bytes),
                Err(JvError::HttpStatus { status: 404, .. }) => continue,
                Err(e) => {
                    warn!("artifact fetch error from {}: {}", repo.id, e);
                    continue;
                }
            }
        }

        Err(JvError::DependencyNotFound {
            coord: artifact.coordinate.to_string(),
        })
    }

    /// Add an additional repository at runtime (lower priority than existing ones).
    pub fn add_repository(&mut self, repo: Repository) {
        // Because we use Arc we need to rebuild. For simplicity in v1 we accept the cost.
        let mut repos = self.inner.repositories.clone();
        repos.push(repo);
        *Arc::get_mut(&mut self.inner).unwrap() = Inner {
            client: self.inner.client.clone(),
            repositories: repos,
        };
    }

    /// Fetch the raw POM XML for a coordinate from the first repository that has it.
    pub async fn fetch_pom(&self, coord: &MavenCoordinate) -> Result<String> {
        let path = format!(
            "{}/{}/{}-{}.pom",
            coord.path(),
            coord.version,
            coord.artifact_id,
            coord.version
        );

        for repo in &self.inner.repositories {
            let url = repo.url.join(&path).map_err(|e| JvError::Other(e.into()))?;

            debug!("fetching POM from {}: {}", repo.id, url);

            match self.fetch_text(&url, &repo.id).await {
                Ok(body) => return Ok(body),
                Err(JvError::HttpStatus { status: 404, .. }) => {
                    debug!("POM not found in repository {}", repo.id);
                    continue;
                }
                Err(e) => {
                    warn!("error fetching from {}: {}", repo.id, e);
                    // try next repo
                    continue;
                }
            }
        }

        Err(JvError::DependencyNotFound {
            coord: coord.to_string(),
        })
    }

    /// Fetch `maven-metadata.xml` for a given group:artifact.
    pub async fn fetch_metadata(&self, group_id: &str, artifact_id: &str) -> Result<MavenMetadata> {
        let path = format!(
            "{}/{}/maven-metadata.xml",
            group_id.replace('.', "/"),
            artifact_id
        );

        for repo in &self.inner.repositories {
            let url = repo.url.join(&path).map_err(|e| JvError::Other(e.into()))?;

            debug!("fetching metadata from {}: {}", repo.id, url);

            match self.fetch_text(&url, &repo.id).await {
                Ok(body) => return Self::parse_metadata(&body, group_id, artifact_id),
                Err(JvError::HttpStatus { status: 404, .. }) => continue,
                Err(e) => {
                    warn!("metadata fetch error from {}: {}", repo.id, e);
                    continue;
                }
            }
        }

        Err(JvError::DependencyNotFound {
            coord: format!("{}:{}", group_id, artifact_id),
        })
    }

    async fn fetch_text(&self, url: &Url, repo_id: &str) -> Result<String> {
        let resp = self
            .inner
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| JvError::Http {
                repo: repo_id.to_string(),
                source: e,
            })?;

        let status = resp.status();
        if status == StatusCode::OK {
            resp.text().await.map_err(|e| JvError::Http {
                repo: repo_id.to_string(),
                source: e,
            })
        } else {
            Err(JvError::HttpStatus {
                status: status.as_u16(),
                url: url.to_string(),
            })
        }
    }

    async fn fetch_bytes(&self, url: &Url, repo_id: &str) -> Result<Vec<u8>> {
        let resp = self
            .inner
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| JvError::Http {
                repo: repo_id.to_string(),
                source: e,
            })?;

        let status = resp.status();
        if status == StatusCode::OK {
            resp.bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| JvError::Http {
                    repo: repo_id.to_string(),
                    source: e,
                })
        } else {
            Err(JvError::HttpStatus {
                status: status.as_u16(),
                url: url.to_string(),
            })
        }
    }

    fn parse_metadata(xml: &str, group_id: &str, artifact_id: &str) -> Result<MavenMetadata> {
        // Lightweight manual parsing for the common fields we need.
        // A full solution would use quick-xml + a metadata struct.
        let mut meta = MavenMetadata {
            group_id: group_id.to_string(),
            artifact_id: artifact_id.to_string(),
            ..Default::default()
        };

        // Very small state machine to extract <version>...</version> and <latest>/<release>
        for line in xml.lines() {
            let line = line.trim();
            if line.starts_with("<latest>") {
                if let Some(v) = Self::extract_tag(line, "latest") {
                    meta.latest = Version::parse(&v).ok();
                }
            } else if line.starts_with("<release>") {
                if let Some(v) = Self::extract_tag(line, "release") {
                    meta.release = Version::parse(&v).ok();
                }
            } else if line.starts_with("<version>") {
                if let Some(v) = Self::extract_tag(line, "version") {
                    if let Ok(ver) = Version::parse(&v) {
                        meta.versions.push(ver);
                    }
                }
            }
        }

        Ok(meta)
    }

    fn extract_tag(line: &str, tag: &str) -> Option<String> {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        if let Some(start) = line.find(&open) {
            if let Some(end) = line.find(&close) {
                return Some(line[start + open.len()..end].to_string());
            }
        }
        None
    }
}

impl Default for RepositoryClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_central_url_is_valid() {
        let central = Repository::maven_central();
        assert!(central.url.as_str().contains("maven.apache.org"));
    }

    #[tokio::test]
    async fn can_fetch_a_well_known_pom() {
        // This test requires network. In CI it may be skipped or mocked.
        let client = RepositoryClient::new();
        let coord = MavenCoordinate::new(
            "org.apache.commons",
            "commons-lang3",
            Version::parse("3.14.0").unwrap(),
        );

        let pom = client.fetch_pom(&coord).await;
        assert!(
            pom.is_ok(),
            "should successfully fetch commons-lang3 POM from Maven Central"
        );
        let pom_xml = pom.unwrap();
        assert!(
            pom_xml.contains("commons-lang3"),
            "POM should contain the artifact name"
        );
    }
}
