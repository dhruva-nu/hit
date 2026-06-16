//! Per-operation builders: parameters, request bodies, and responses.

use serde_json::Value;

use crate::model::{BodySpec, Param, ParamLocation};
use crate::spec::raw::RawParameter;
use crate::spec::{normalize, resolve};

pub(super) fn build_param(document: &Value, param_value: &Value) -> Option<Param> {
    let (resolved, _) = resolve::deref(document, param_value);
    let raw: RawParameter = serde_json::from_value(resolved.clone()).ok()?;
    let location = match raw.location.as_str() {
        "path" => ParamLocation::Path,
        "query" => ParamLocation::Query,
        "header" => ParamLocation::Header,
        // cookie params are out of scope; auth handles its own headers
        _ => return None,
    };
    let normalized = raw
        .schema
        .as_ref()
        .map(|s| normalize::normalize(document, s))
        .unwrap_or_else(normalize::Normalized::any);
    Some(Param {
        name: raw.name,
        location,
        // Path params are always required regardless of what the spec says.
        required: raw.required || location == ParamLocation::Path,
        nullable: normalized.nullable,
        schema: normalized.node,
        default: normalized.default,
        description: raw.description.or(normalized.description),
    })
}

/// Pick the request content type we can drive: JSON preferred, then form
/// variants. Endpoints with only unsupported content degrade to Any-schema JSON.
pub(super) fn build_body(
    document: &Value,
    request_body: &Value,
    endpoint_id: &str,
) -> Option<BodySpec> {
    let (resolved, _) = resolve::deref(document, request_body);
    let content = resolved.get("content")?.as_object()?;
    let required = resolved
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let preferred = ["application/json"];
    let acceptable = ["application/x-www-form-urlencoded", "multipart/form-data"];

    let chosen = preferred
        .iter()
        .chain(acceptable.iter())
        .find_map(|ct| content.get_key_value(*ct))
        .or_else(|| content.iter().find(|(ct, _)| ct.contains("json")))
        .or_else(|| {
            tracing::warn!(
                endpoint = endpoint_id,
                content_types = ?content.keys().collect::<Vec<_>>(),
                "no supported request content type; using first declared"
            );
            content.iter().next()
        })?;

    let (content_type, media) = chosen;
    let normalized = media
        .get("schema")
        .map(|s| normalize::normalize(document, s))
        .unwrap_or_else(normalize::Normalized::any);

    Some(BodySpec {
        content_type: content_type.clone(),
        schema: normalized.node,
        required,
    })
}

/// One declared response: description plus the normalized JSON schema when
/// the response declares JSON content.
pub(super) fn build_response(
    document: &Value,
    status: &str,
    response: &Value,
) -> crate::model::ResponseSpec {
    let (resolved, _) = resolve::deref(document, response);
    let description = resolved
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string);
    let schema = resolved
        .get("content")
        .and_then(Value::as_object)
        .and_then(|content| {
            content.get("application/json").or_else(|| {
                content
                    .iter()
                    .find(|(ct, _)| ct.contains("json"))
                    .map(|(_, v)| v)
            })
        })
        .and_then(|media| media.get("schema"))
        .map(|s| normalize::normalize(document, s).node);
    crate::model::ResponseSpec {
        status: status.to_string(),
        description,
        schema,
    }
}
