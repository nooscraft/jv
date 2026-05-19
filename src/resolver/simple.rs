//! Extremely simple "resolver" for the initial end-to-end demo.
//!
//! It only handles direct dependencies declared in the root POM. The next
//! iteration will add recursive transitive collection + conflict resolution.

use crate::cache::CacheManager;
use crate::download::{DownloadResult, ParallelDownloader};
use crate::error::Result;
use crate::models::{Artifact, MavenCoordinate, ResolvedDependency, Scope};
use crate::parser::Pom;
use crate::repository::RepositoryClient;
use std::path::Path;

/// Resolve only the direct dependencies declared in the given POM file.
/// Returns the list of successfully downloaded artifacts and a resolved set.
pub async fn resolve_direct(
    pom_path: &Path,
    extra_repos: &[String],
) -> Result<(Vec<ResolvedDependency>, Vec<DownloadResult>)> {
    let xml = std::fs::read_to_string(pom_path)?;
    let pom = Pom::parse(&xml)?;

    // Build client (Maven Central + any extra repos)
    let mut client = RepositoryClient::new();
    for url in extra_repos {
        if let Ok(repo) = crate::repository::Repository::new("extra", url) {
            client.add_repository(repo);
        }
    }

    let cache = CacheManager::new()?;
    let downloader = ParallelDownloader::new(client.clone(), cache.clone());

    // Convert direct dependencies into artifacts we want to fetch
    let artifacts: Vec<Artifact> = pom
        .dependencies
        .iter()
        .filter(|d| d.scope.is_transitive()) // compile + runtime for a normal build
        .map(|d| {
            let mut a = Artifact::jar(d.coordinate.clone());
            a.classifier = d.classifier.clone();
            if let Some(t) = &d.r#type {
                if t != "jar" {
                    a.extension = t.clone();
                }
            }
            a
        })
        .collect();

    let results = downloader.download_all(&artifacts).await;

    // Build a very basic resolved list (no transitives yet)
    let resolved: Vec<ResolvedDependency> = results
        .iter()
        .filter_map(|r| {
            if r.path.is_some() {
                Some(ResolvedDependency {
                    coordinate: r.artifact.coordinate.clone(),
                    scope: Scope::Compile,
                    optional: false,
                    depended_by: Some(pom.coordinate.clone()),
                    artifacts: vec![r.artifact.clone()],
                })
            } else {
                None
            }
        })
        .collect();

    Ok((resolved, results))
}
