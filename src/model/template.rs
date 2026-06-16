//! RequestTemplate generation: turns an `Endpoint` into a fill-in-the-blanks
//! request description consumed by `hit template`, the MCP
//! `get_request_template` tool, and the TUI form seed.

use serde::Serialize;
use serde_json::Value;

use super::example::{example_value, placeholder};
use super::{Endpoint, ParamLocation, SchemaNode};

#[derive(Debug, Clone, Serialize)]
pub struct RequestTemplate {
    pub endpoint_id: String,
    pub method: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub path_params: Vec<TemplateField>,
    pub query_params: Vec<TemplateField>,
    pub header_params: Vec<TemplateField>,
    /// Example JSON body: defaults filled in, `<type:format>` placeholders
    /// elsewhere. Optional fields are present but listed in `optional_paths`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
    /// The normalized body schema, for consumers that want the full shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_schema: Option<SchemaNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_content_type: Option<String>,
    /// Dotted body paths that may be omitted entirely (e.g. "address.line2").
    pub optional_paths: Vec<String>,
    /// Dotted body paths that accept JSON null.
    pub nullable_paths: Vec<String>,
    pub auth_required: bool,
    /// Declared responses with example bodies, in spec order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub responses: Vec<TemplateResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Example body generated from the response schema (placeholders for
    /// scalars), when the spec declares JSON content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateField {
    pub name: String,
    pub required: bool,
    pub nullable: bool,
    /// Placeholder or default value.
    pub value: Value,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub fn build_template(endpoint: &Endpoint) -> RequestTemplate {
    let mut optional_paths = Vec::new();
    let mut nullable_paths = Vec::new();

    let body = endpoint
        .body
        .as_ref()
        .map(|b| example_value(&b.schema, "", &mut optional_paths, &mut nullable_paths, 0));

    RequestTemplate {
        endpoint_id: endpoint.id.clone(),
        method: endpoint.method.clone(),
        path: endpoint.path.clone(),
        summary: endpoint.summary.clone(),
        path_params: param_fields(endpoint, ParamLocation::Path),
        query_params: param_fields(endpoint, ParamLocation::Query),
        header_params: param_fields(endpoint, ParamLocation::Header),
        body,
        body_schema: endpoint.body.as_ref().map(|b| b.schema.clone()),
        body_content_type: endpoint.body.as_ref().map(|b| b.content_type.clone()),
        optional_paths,
        nullable_paths,
        auth_required: endpoint.auth_required,
        responses: response_templates(endpoint),
    }
}

/// Build the template fields for all params at one location.
fn param_fields(endpoint: &Endpoint, location: ParamLocation) -> Vec<TemplateField> {
    endpoint
        .params_in(location)
        .map(|p| TemplateField {
            name: p.name.clone(),
            required: p.required,
            nullable: p.nullable,
            value: p.default.clone().unwrap_or_else(|| placeholder(&p.schema)),
            kind: p.schema.kind_label(),
            description: p.description.clone(),
        })
        .collect()
}

/// Build the example-bearing response templates, in spec order.
fn response_templates(endpoint: &Endpoint) -> Vec<TemplateResponse> {
    endpoint
        .responses
        .iter()
        .map(|r| TemplateResponse {
            status: r.status.clone(),
            description: r.description.clone(),
            example: r.schema.as_ref().map(example_of),
        })
        .collect()
}

/// Standalone example generator for a schema (used for response previews).
pub fn example_of(node: &SchemaNode) -> Value {
    example_value(node, "", &mut Vec::new(), &mut Vec::new(), 0)
}
