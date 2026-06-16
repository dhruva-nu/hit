//! The build step: turn a raw OpenAPI document into the normalized `ApiSpec`
//! domain model (endpoints, params, bodies, responses, tag groups).

mod components;
mod tags;

use serde_json::Value;

use crate::error::SpecError;
use crate::model::{ApiSpec, Endpoint};
use crate::spec::raw::{RawOperation, RawPathItem, RawSpec};

use components::{build_body, build_param, build_response};
use tags::group_tags;

/// Build the normalized domain model from a raw OpenAPI document.
pub fn build(document: &Value) -> Result<ApiSpec, SpecError> {
    let raw: RawSpec =
        serde_json::from_value(document.clone()).map_err(|e| SpecError::Parse(e.to_string()))?;
    ensure_has_paths(&raw, document)?;

    let spec_level_security = raw
        .security
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let mut endpoints = Vec::new();
    for (path, item) in &raw.paths {
        for (method, op) in item.operations() {
            endpoints.push(build_endpoint(
                document,
                path,
                method,
                op,
                item,
                spec_level_security,
            ));
        }
    }

    let tags = group_tags(&raw, &endpoints);
    Ok(ApiSpec {
        title: raw.info.title,
        version: raw.info.version,
        openapi_version: raw.openapi,
        tags,
        endpoints,
    })
}

/// Reject documents that aren't recognizably OpenAPI (no `paths` object).
fn ensure_has_paths(raw: &RawSpec, document: &Value) -> Result<(), SpecError> {
    if raw.paths.is_empty()
        && !document
            .get("paths")
            .map(|p| p.is_object())
            .unwrap_or(false)
    {
        return Err(SpecError::Parse(
            "document has no 'paths' object — is this an OpenAPI spec?".into(),
        ));
    }
    Ok(())
}

/// Build one endpoint from a path item's operation, folding in path-level
/// parameters and resolving the effective auth requirement.
fn build_endpoint(
    document: &Value,
    path: &str,
    method: &str,
    op: &RawOperation,
    item: &RawPathItem,
    spec_level_security: bool,
) -> Endpoint {
    let id = op
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("{method} {path}"));

    let mut params = Vec::new();
    for param_value in item.parameters.iter().chain(op.parameters.iter()) {
        match build_param(document, param_value) {
            Some(p) => params.push(p),
            None => tracing::warn!(endpoint = id, "skipping unparseable parameter"),
        }
    }

    let body = op
        .request_body
        .as_ref()
        .and_then(|rb| build_body(document, rb, &id));

    let auth_required = match &op.security {
        Some(reqs) => !reqs.is_empty(),
        None => spec_level_security,
    };

    let responses = op
        .responses
        .iter()
        .map(|(status, response)| build_response(document, status, response))
        .collect();

    Endpoint {
        id,
        method: method.to_string(),
        path: path.to_string(),
        summary: op.summary.clone(),
        description: op.description.clone(),
        tags: op.tags.clone(),
        deprecated: op.deprecated,
        params,
        body,
        auth_required,
        responses,
    }
}
