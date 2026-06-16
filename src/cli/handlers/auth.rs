//! `hit login` / `hit logout` handlers.

use std::sync::Arc;

use serde_json::json;

use crate::AppServices;
use crate::auth::{AuthManager, CliInteractor};
use crate::cli::output::CommandOutput;
use crate::config;
use crate::error::HitError;

pub(crate) async fn login_cmd(
    project_name: &str,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    let project = config::project(&services.config, project_name)?;
    let auth = AuthManager::for_project(
        project_name,
        project,
        services.settings(),
        &services.paths,
        services.client.clone(),
        Arc::new(CliInteractor),
        false,
    )?
    .ok_or_else(|| HitError::Other(format!("project '{project_name}' has no auth configured")))?;

    auth.invalidate().await; // force a fresh login
    auth.bearer().await?;
    let expiry = auth.cached_expiry();
    let human = match expiry {
        Some(exp) => {
            let remaining = exp.saturating_sub(crate::auth::token_store::now_unix());
            format!("logged in to '{project_name}' (token expires in {remaining}s)")
        }
        None => format!("logged in to '{project_name}' (token has no visible expiry)"),
    };
    Ok(CommandOutput::ok(
        json!({"project": project_name, "expires_at_unix": expiry}),
        human,
    ))
}

pub(crate) fn logout_cmd(
    project_name: &str,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    config::project(&services.config, project_name)?;
    let store = crate::auth::new_token_store(
        services.settings().token_store,
        services.paths.token_dir.clone(),
    )?;
    store.clear(project_name)?;
    Ok(CommandOutput::ok(
        json!({"project": project_name}),
        format!("cleared cached token for '{project_name}'"),
    ))
}
