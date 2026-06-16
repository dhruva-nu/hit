//! Spec-reading handlers: tags, endpoints, template, and cache refresh.

use serde_json::{Value, json};

use crate::cli::handlers::load_spec;
use crate::cli::output::CommandOutput;
use crate::config;
use crate::error::HitError;
use crate::model::build_template;
use crate::{AppServices, spec};

pub(crate) async fn tags_cmd(
    project: &str,
    no_cache: bool,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    let loaded = load_spec(project, no_cache, services).await?;
    let rows: Vec<Value> = loaded
        .spec
        .tags
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "endpoint_count": t.endpoint_ids.len(),
            })
        })
        .collect();
    let human = loaded
        .spec
        .tags
        .iter()
        .map(|t| {
            format!(
                "{:<24} {:>3} endpoints  {}",
                t.name,
                t.endpoint_ids.len(),
                t.description.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(CommandOutput::ok(json!(rows), human))
}

pub(crate) async fn template_cmd(
    project: &str,
    endpoint: &str,
    no_cache: bool,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    let loaded = load_spec(project, no_cache, services).await?;
    let endpoint = loaded.spec.find_endpoint(endpoint)?;
    let template = build_template(endpoint);
    let data = serde_json::to_value(&template).map_err(|e| HitError::Other(e.to_string()))?;
    let human = serde_json::to_string_pretty(&data).unwrap_or_default();
    Ok(CommandOutput::ok(data, human))
}

pub(crate) async fn refresh_cmd(
    project_name: &str,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    let project = config::project(&services.config, project_name)?;
    let loaded = spec::refresh(
        &services.client,
        project_name,
        project,
        &services.paths.spec_cache_dir,
    )
    .await?;
    let data = json!({
        "title": loaded.spec.title,
        "version": loaded.spec.version,
        "openapi_version": loaded.spec.openapi_version,
        "tags": loaded.spec.tags.len(),
        "endpoints": loaded.spec.endpoints.len(),
    });
    let human = format!(
        "refreshed '{project_name}': {} v{} — {} endpoints across {} tags",
        loaded.spec.title,
        loaded.spec.version,
        loaded.spec.endpoints.len(),
        loaded.spec.tags.len()
    );
    Ok(CommandOutput::ok(data, human))
}
