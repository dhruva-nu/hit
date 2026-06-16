//! `execute_request` tool body and its MCP-specific auth resolution.

use std::sync::Arc;

use rmcp::model::{CallToolResult, ErrorData};

use crate::auth::{AuthManager, DenyInteractor};
use crate::error::HitError;
use crate::http::{RequestArgs, RequestExecutor};
use crate::mcp::params::ExecuteParams;
use crate::mcp::tools::{load_spec, tool_error};
use crate::{AppServices, config};

pub(crate) async fn execute_request(
    services: &AppServices,
    params: ExecuteParams,
) -> Result<CallToolResult, ErrorData> {
    let loaded = load_spec(services, &params.project).await?;
    let endpoint = loaded
        .spec
        .find_endpoint(&params.endpoint)
        .map_err(|e| tool_error(e.into()))?
        .clone();
    let project =
        config::project(&services.config, &params.project).map_err(|e| tool_error(e.into()))?;

    let args = RequestArgs::from_parts(
        params.path_params.unwrap_or_default(),
        params
            .query_params
            .unwrap_or_default()
            .into_iter()
            .collect(),
        params.headers.unwrap_or_default().into_iter().collect(),
        params.body,
        params.no_auth,
    );

    let auth = build_auth(services, &params.project, project, &args)?;
    let executor = RequestExecutor {
        client: &services.client,
        project,
        auth: auth.as_ref(),
    };
    let response = executor
        .execute(&endpoint, &args)
        .await
        .map_err(tool_error)?;
    serde_json::to_value(&response)
        .map(CallToolResult::structured)
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))
}

/// Resolve auth for an MCP request, never launching a browser.
fn build_auth(
    services: &AppServices,
    project_name: &str,
    project: &config::ProjectConfig,
    args: &RequestArgs,
) -> Result<Option<AuthManager>, ErrorData> {
    let interactor = Arc::new(DenyInteractor {
        instruction: format!(
            "interactive auth required — run `hit login {project_name}` in a terminal, then retry"
        ),
    });
    let auth = AuthManager::for_project(
        project_name,
        project,
        services.settings(),
        &services.paths,
        services.client.clone(),
        interactor,
        true,
    )
    .map_err(|e| tool_error(e.into()))?;

    if let Some(manager) = &auth
        && !manager.supports_headless()
        && manager.cached_expiry().is_none()
        && !args.no_auth
    {
        return Err(tool_error(HitError::Auth(
            crate::error::AuthError::InteractionRequired(format!(
                "project '{project_name}' uses browser-based OAuth and has no cached token — run \
                 `hit login {project_name}` in a terminal first"
            )),
        )));
    }
    Ok(auth)
}
