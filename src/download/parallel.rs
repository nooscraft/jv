//! High-concurrency artifact downloader with cache integration and progress bars.
//!
//! Uses a semaphore to limit concurrent downloads (default 16) while still
//! allowing the overall resolution to feel extremely fast.

use crate::cache::CacheManager;

use crate::models::Artifact;
use crate::repository::RepositoryClient;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, info};

const DEFAULT_CONCURRENCY: usize = 16;
const PROGRESS_TEMPLATE: &str =
    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}";

/// Result of a single artifact download attempt.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub artifact: Artifact,
    pub path: Option<PathBuf>,
    pub cached: bool,
    pub error: Option<String>,
}

/// High-level parallel downloader.
pub struct ParallelDownloader {
    client: RepositoryClient,
    cache: CacheManager,
    concurrency: usize,
}

impl ParallelDownloader {
    pub fn new(client: RepositoryClient, cache: CacheManager) -> Self {
        Self {
            client,
            cache,
            concurrency: DEFAULT_CONCURRENCY,
        }
    }

    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.concurrency = n.max(1);
        self
    }

    /// Download (or reuse from cache) a batch of artifacts concurrently.
    ///
    /// Returns a vector of results in the same order as the input `artifacts`.
    /// A nice multi-progress bar is shown on the terminal.
    pub async fn download_all(&self, artifacts: &[Artifact]) -> Vec<DownloadResult> {
        if artifacts.is_empty() {
            return vec![];
        }

        let multi = MultiProgress::new();
        let main_pb = multi.add(ProgressBar::new(artifacts.len() as u64));
        main_pb.set_style(
            ProgressStyle::default_bar()
                .template(PROGRESS_TEMPLATE)
                .unwrap()
                .progress_chars("##-"),
        );
        main_pb.enable_steady_tick(Duration::from_millis(100));

        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let client = self.client.clone();
        let cache = self.cache.clone();

        let results: Vec<_> = stream::iter(artifacts.to_vec())
            .enumerate()
            .map(|(_idx, artifact)| {
                let sem = semaphore.clone();
                let client = client.clone();
                let cache = cache.clone();
                let multi = multi.clone();
                let main_pb = main_pb.clone();

                async move {
                    let _permit = sem.acquire().await.expect("semaphore closed");

                    let pb = multi.insert_after(&main_pb, ProgressBar::new_spinner());
                    pb.set_message(format!(
                        "{}:{}",
                        artifact.coordinate.artifact_id, artifact.coordinate.version
                    ));

                    let res = Self::download_one(&client, &cache, &artifact, &pb).await;

                    pb.finish_and_clear();
                    main_pb.inc(1);

                    if res.cached {
                        debug!("reused from cache: {}", artifact.coordinate);
                    } else if res.path.is_some() {
                        debug!("downloaded: {}", artifact.coordinate);
                    }

                    res
                }
            })
            .buffer_unordered(self.concurrency)
            .collect()
            .await;

        main_pb.finish_with_message("download complete");
        info!("Finished downloading {} artifacts", artifacts.len());

        results
    }

    async fn download_one(
        client: &RepositoryClient,
        cache: &CacheManager,
        artifact: &Artifact,
        pb: &ProgressBar,
    ) -> DownloadResult {
        // 1. Check cache first (the most common fast path)
        if let Some(path) = cache.get_artifact(artifact) {
            return DownloadResult {
                artifact: artifact.clone(),
                path: Some(path),
                cached: true,
                error: None,
            };
        }

        pb.set_message(format!("fetching {}", artifact.coordinate));

        // 2. Use the repository client's fetch_artifact_bytes (tries all repos)
        match client.fetch_artifact_bytes(artifact).await {
            Ok(bytes) => match cache.put_artifact(artifact, &bytes) {
                Ok(path) => DownloadResult {
                    artifact: artifact.clone(),
                    path: Some(path),
                    cached: false,
                    error: None,
                },
                Err(e) => DownloadResult {
                    artifact: artifact.clone(),
                    path: None,
                    cached: false,
                    error: Some(e.to_string()),
                },
            },
            Err(e) => DownloadResult {
                artifact: artifact.clone(),
                path: None,
                cached: false,
                error: Some(e.to_string()),
            },
        }
    }
}
