//! `hit endpoints` handler.

use serde_json::{Value, json};

use crate::AppServices;
use crate::cli::handlers::load_spec;
use crate::cli::output::CommandOutput;
use crate::error::HitError;
use crate::model::Endpoint;

pub(crate) async fn endpoints_cmd(
    project: &str,
    tag: Option<&str>,
    search: Option<&str>,
    no_cache: bool,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    let loaded = load_spec(project, no_cache, services).await?;
    let spec = &loaded.spec;

    let in_tag: Option<Vec<&str>> = match tag {
        Some(tag_name) => Some(
            spec.tag(tag_name)?
                .endpoint_ids
                .iter()
                .map(String::as_str)
                .collect(),
        ),
        None => None,
    };

    let needle = search.map(str::to_ascii_lowercase);
    let endpoints: Vec<_> = spec
        .endpoints
        .iter()
        .filter(|e| {
            in_tag
                .as_ref()
                .map(|ids| ids.contains(&e.id.as_str()))
                .unwrap_or(true)
        })
        .filter(|e| needle.as_ref().is_none_or(|n| matches_needle(e, n)))
        .collect();

    Ok(CommandOutput::ok(
        endpoints_json(&endpoints),
        endpoints_human(&endpoints),
    ))
}

fn matches_needle(endpoint: &Endpoint, needle: &str) -> bool {
    endpoint.id.to_ascii_lowercase().contains(needle)
        || endpoint.path.to_ascii_lowercase().contains(needle)
        || endpoint
            .summary
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains(needle)
}

fn endpoints_json(endpoints: &[&Endpoint]) -> Value {
    let rows: Vec<Value> = endpoints
        .iter()
        .map(|e| {
            json!({
                "id": e.id,
                "method": e.method,
                "path": e.path,
                "summary": e.summary,
                "tags": e.tags,
                "has_body": e.body.is_some(),
                "auth_required": e.auth_required,
                "deprecated": e.deprecated,
            })
        })
        .collect();
    json!(rows)
}

fn endpoints_human(endpoints: &[&Endpoint]) -> String {
    endpoints
        .iter()
        .map(|e| {
            format!(
                "{:<7} {:<40} {}{}",
                e.method,
                e.path,
                e.summary.as_deref().unwrap_or(&e.id),
                if e.deprecated { "  [deprecated]" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
