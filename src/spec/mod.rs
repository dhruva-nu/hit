//! Spec acquisition (live fetch / disk fallback / cache) and the build step
//! that turns raw OpenAPI into the normalized `ApiSpec` domain model.

pub mod adapter;
pub mod build;
pub mod normalize;
pub mod raw;
pub mod resolve;

mod cache;

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::config::{ProjectConfig, Settings};
use crate::error::SpecError;
use crate::model::ApiSpec;

use cache::{cache_age, cache_file, fetch_live, read_cache, read_spec_file, spec_url, write_cache};

// `build` the function lives in the value namespace and coexists with the
// `build` module; re-export it so `crate::spec::build` keeps resolving to the
// function for external callers (tui, integration tests).
pub use build::build;

/// Where a spec document ultimately came from, surfaced in CLI/MCP output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecOrigin {
    Cache,
    Live,
    File,
    StaleCache,
}

pub struct LoadedSpec {
    pub spec: ApiSpec,
    pub origin: SpecOrigin,
    /// The raw document, kept for schema normalization context ($ref targets).
    pub document: Value,
}

/// Load a project's spec: fresh cache -> live fetch -> disk file -> stale cache.
pub async fn load(
    client: &reqwest::Client,
    project_name: &str,
    project: &ProjectConfig,
    settings: &Settings,
    cache_dir: &Path,
    no_cache: bool,
) -> Result<LoadedSpec, SpecError> {
    let cache_path = cache_file(cache_dir, project_name);
    let ttl = Duration::from_secs(settings.spec_cache_ttl_secs);

    if !no_cache && let Some(loaded) = load_fresh_cache(&cache_path, ttl)? {
        return Ok(loaded);
    }

    let url = spec_url(project);
    match fetch_live(client, &url).await {
        Ok(document) => load_live(&document, &cache_path),
        Err(fetch_err) => {
            tracing::warn!(url, error = %fetch_err, "live spec fetch failed; trying fallbacks");
            load_fallback(project, project_name, &cache_path, fetch_err)
        }
    }
}

/// Serve a still-fresh cached document, if one exists within the TTL.
fn load_fresh_cache(cache_path: &Path, ttl: Duration) -> Result<Option<LoadedSpec>, SpecError> {
    let Some(entry) = read_cache(cache_path) else {
        return Ok(None);
    };
    if !cache_age(&entry).map(|age| age < ttl).unwrap_or(false) {
        return Ok(None);
    }
    let spec = build(&entry.openapi)?;
    Ok(Some(LoadedSpec {
        spec,
        origin: SpecOrigin::Cache,
        document: entry.openapi,
    }))
}

/// Build from a freshly fetched document and refresh the cache.
fn load_live(document: &Value, cache_path: &Path) -> Result<LoadedSpec, SpecError> {
    let spec = build(document)?;
    write_cache(cache_path, document);
    Ok(LoadedSpec {
        spec,
        origin: SpecOrigin::Live,
        document: document.clone(),
    })
}

/// Live fetch failed: fall back to a configured spec file, then to a stale
/// cache, then give up.
fn load_fallback(
    project: &ProjectConfig,
    project_name: &str,
    cache_path: &Path,
    fetch_err: String,
) -> Result<LoadedSpec, SpecError> {
    if let Some(spec_file) = &project.spec_file {
        let document = read_spec_file(spec_file)?;
        let spec = build(&document)?;
        return Ok(LoadedSpec {
            spec,
            origin: SpecOrigin::File,
            document,
        });
    }
    if let Some(entry) = read_cache(cache_path) {
        tracing::warn!("serving stale cached spec");
        let spec = build(&entry.openapi)?;
        return Ok(LoadedSpec {
            spec,
            origin: SpecOrigin::StaleCache,
            document: entry.openapi,
        });
    }
    Err(SpecError::Unavailable {
        project: project_name.to_string(),
        detail: fetch_err,
    })
}

/// Force a refetch and recache, bypassing all fallbacks.
pub async fn refresh(
    client: &reqwest::Client,
    project_name: &str,
    project: &ProjectConfig,
    cache_dir: &Path,
) -> Result<LoadedSpec, SpecError> {
    let url = spec_url(project);
    let document = fetch_live(client, &url)
        .await
        .map_err(|message| SpecError::Fetch { url, message })?;
    let spec = build(&document)?;
    write_cache(&cache_file(cache_dir, project_name), &document);
    Ok(LoadedSpec {
        spec,
        origin: SpecOrigin::Live,
        document,
    })
}
