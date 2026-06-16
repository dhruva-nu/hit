//! Parameter structs for the MCP tools.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, JsonSchema)]
pub(crate) struct ProjectParam {
    /// Registered project name (see list_projects).
    pub project: String,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct ListEndpointsParams {
    /// Registered project name (see list_projects).
    pub project: String,
    /// Restrict to one OpenAPI tag (see list_tags).
    #[serde(default)]
    pub tag: Option<String>,
    /// Case-insensitive substring match on id, path, or summary.
    #[serde(default)]
    pub search: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct EndpointParams {
    /// Registered project name (see list_projects).
    pub project: String,
    /// Endpoint id (operation_id) or "METHOD /path", e.g. "POST /users/".
    pub endpoint: String,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct ExecuteParams {
    /// Registered project name (see list_projects).
    pub project: String,
    /// Endpoint id (operation_id) or "METHOD /path", e.g. "POST /users/".
    pub endpoint: String,
    /// JSON request body. Call get_request_template first to learn the shape;
    /// omit optional fields you don't need (listed in optional_paths).
    #[serde(default)]
    pub body: Option<Value>,
    /// Values for the {placeholders} in the endpoint path.
    #[serde(default)]
    pub path_params: Option<BTreeMap<String, String>>,
    /// Query-string parameters.
    #[serde(default)]
    pub query_params: Option<BTreeMap<String, String>>,
    /// Extra request headers.
    #[serde(default)]
    pub headers: Option<BTreeMap<String, String>>,
    /// Skip authentication for this request.
    #[serde(default)]
    pub no_auth: bool,
}
