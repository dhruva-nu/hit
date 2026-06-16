//! MCP server mode (`hit mcp`): the same capabilities as the headless CLI,
//! exposed as MCP tools over stdio for AI agents.
//!
//! stdout is protocol — logging goes to file only (set up in main).

mod exec;
mod params;
mod tools;

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde_json::{Value, json};

use crate::AppServices;
use crate::model::build_template;

use params::{EndpointParams, ExecuteParams, ListEndpointsParams, ProjectParam};
use tools::tool_error;

pub async fn serve(services: AppServices) -> i32 {
    let server = HitpointServer {
        services: Arc::new(services),
    };
    let running = match server.serve(rmcp::transport::stdio()).await {
        Ok(running) => running,
        Err(e) => {
            tracing::error!(error = %e, "failed to start MCP server");
            return 1;
        }
    };
    if let Err(e) = running.waiting().await {
        tracing::error!(error = %e, "MCP server terminated abnormally");
        return 1;
    }
    0
}

struct HitpointServer {
    services: Arc<AppServices>,
}

#[tool_router]
impl HitpointServer {
    #[tool(
        name = "list_projects",
        description = "List the registered API projects this machine can test. Start here."
    )]
    async fn list_projects(&self) -> CallToolResult {
        let projects: Vec<Value> = self
            .services
            .config
            .projects
            .iter()
            .map(|(name, p)| {
                json!({
                    "name": name,
                    "base_url": p.base_url.as_str(),
                    "auth_type": p.auth.as_ref().map(|a| a.type_name()),
                })
            })
            .collect();
        CallToolResult::structured(json!({"projects": projects}))
    }

    #[tool(
        name = "list_tags",
        description = "List a project's OpenAPI tags (endpoint groups) with endpoint counts."
    )]
    async fn list_tags(
        &self,
        Parameters(params): Parameters<ProjectParam>,
    ) -> Result<CallToolResult, ErrorData> {
        let loaded = tools::load_spec(&self.services, &params.project).await?;
        let tags: Vec<Value> = loaded
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
        Ok(CallToolResult::structured(json!({"tags": tags})))
    }

    #[tool(
        name = "list_endpoints",
        description = "List a project's endpoints (id, method, path, summary), optionally \
                       filtered by tag or search string. Use the id with \
                       get_request_template / execute_request."
    )]
    async fn list_endpoints(
        &self,
        Parameters(params): Parameters<ListEndpointsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        tools::list_endpoints(&self.services, params).await
    }

    #[tool(
        name = "get_request_template",
        description = "Get a fill-in-the-blanks request template for an endpoint: an example \
                       body with placeholders, the body schema, required path/query/header \
                       params, plus optional_paths (droppable fields) and nullable_paths \
                       (fields accepting null). Call this BEFORE execute_request."
    )]
    async fn get_request_template(
        &self,
        Parameters(params): Parameters<EndpointParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let loaded = tools::load_spec(&self.services, &params.project).await?;
        let endpoint = loaded
            .spec
            .find_endpoint(&params.endpoint)
            .map_err(|e| tool_error(e.into()))?;
        let template = build_template(endpoint);
        serde_json::to_value(&template)
            .map(CallToolResult::structured)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }

    #[tool(
        name = "execute_request",
        description = "Execute a request against an endpoint and return {status, headers, body, \
                       latency_ms, url}. Configured project auth (JWT login) is applied \
                       automatically; OAuth projects must be logged in beforehand via \
                       `hit login <project>` in a terminal. A non-2xx HTTP status is a \
                       successful tool call — inspect `status`."
    )]
    async fn execute_request(
        &self,
        Parameters(params): Parameters<ExecuteParams>,
    ) -> Result<CallToolResult, ErrorData> {
        exec::execute_request(&self.services, params).await
    }
}

#[tool_handler]
impl ServerHandler for HitpointServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "hitpoint: test the user's registered API backends. Workflow: \
             list_projects -> list_tags / list_endpoints -> get_request_template \
             -> execute_request. Always fetch the template before executing an \
             endpoint with a body; optional_paths in the template lists fields \
             you may omit, nullable_paths lists fields that accept null."
                .into(),
        );
        info
    }
}
