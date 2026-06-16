//! Free helpers backing the thin MCP tool methods. These hold the long
//! bodies so the macro-annotated `#[tool]` methods in `mod.rs` stay small.

use rmcp::model::{CallToolResult, ErrorData};
use serde_json::{Value, json};

use crate::error::HitError;
use crate::mcp::params::ListEndpointsParams;
use crate::model::Endpoint;
use crate::{AppServices, config, spec};

pub(crate) fn tool_error(error: HitError) -> ErrorData {
    ErrorData::invalid_params(error.to_string(), Some(json!({"kind": error.kind()})))
}

pub(crate) async fn load_spec(
    services: &AppServices,
    project_name: &str,
) -> Result<spec::LoadedSpec, ErrorData> {
    let project =
        config::project(&services.config, project_name).map_err(|e| tool_error(e.into()))?;
    spec::load(
        &services.client,
        project_name,
        project,
        services.settings(),
        &services.paths.spec_cache_dir,
        false,
    )
    .await
    .map_err(|e| tool_error(e.into()))
}

pub(crate) async fn list_endpoints(
    services: &AppServices,
    params: ListEndpointsParams,
) -> Result<CallToolResult, ErrorData> {
    let loaded = load_spec(services, &params.project).await?;
    let spec = &loaded.spec;

    let in_tag: Option<Vec<&str>> = match &params.tag {
        Some(tag_name) => Some(
            spec.tag(tag_name)
                .map_err(|e| tool_error(e.into()))?
                .endpoint_ids
                .iter()
                .map(String::as_str)
                .collect(),
        ),
        None => None,
    };
    let needle = params.search.as_deref().map(str::to_ascii_lowercase);

    let endpoints: Vec<Value> = spec
        .endpoints
        .iter()
        .filter(|e| {
            in_tag
                .as_ref()
                .map(|ids| ids.contains(&e.id.as_str()))
                .unwrap_or(true)
        })
        .filter(|e| needle.as_ref().is_none_or(|n| matches_needle(e, n)))
        .map(endpoint_json)
        .collect();
    Ok(CallToolResult::structured(json!({"endpoints": endpoints})))
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

fn endpoint_json(e: &Endpoint) -> Value {
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
}
