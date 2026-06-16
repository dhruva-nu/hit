//! Disk cache and live-fetch helpers backing [`super::load`] and
//! [`super::refresh`]: where the spec URL lives, how cached documents are
//! read/written atomically, and their freshness.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::ProjectConfig;
use crate::error::SpecError;

/// Cached fetch result on disk: the raw document plus when we got it.
#[derive(Serialize, Deserialize)]
pub(super) struct CacheEntry {
    pub(super) fetched_at_unix: u64,
    pub(super) openapi: Value,
}

pub(super) fn spec_url(project: &ProjectConfig) -> String {
    format!(
        "{}/openapi.json",
        project.base_url.as_str().trim_end_matches('/')
    )
}

pub(super) async fn fetch_live(client: &reqwest::Client, url: &str) -> Result<Value, String> {
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("server returned {status}"));
    }
    response
        .json()
        .await
        .map_err(|e| format!("invalid JSON: {e}"))
}

pub(super) fn cache_file(cache_dir: &Path, project_name: &str) -> PathBuf {
    cache_dir.join(format!("{project_name}.json"))
}

pub(super) fn read_cache(path: &Path) -> Option<CacheEntry> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub(super) fn cache_age(entry: &CacheEntry) -> Option<Duration> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Duration::from_secs(
        now.saturating_sub(entry.fetched_at_unix),
    ))
}

pub(super) fn write_cache(path: &Path, document: &Value) {
    let entry = CacheEntry {
        fetched_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        openapi: document.clone(),
    };
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&entry)?)?;
        std::fs::rename(&tmp, path)
    };
    if let Err(e) = write() {
        tracing::warn!(path = %path.display(), error = %e, "failed to write spec cache");
    }
}

pub(super) fn read_spec_file(path: &Path) -> Result<Value, SpecError> {
    let raw = std::fs::read_to_string(path).map_err(|e| SpecError::Unavailable {
        project: String::new(),
        detail: format!("spec_file {}: {e}", path.display()),
    })?;
    serde_json::from_str(&raw)
        .map_err(|e| SpecError::Parse(format!("spec_file {}: {e}", path.display())))
}
