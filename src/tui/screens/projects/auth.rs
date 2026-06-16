//! Login / logout flows for the projects menu, surfacing results on the
//! status line (and through TUI modals when a credential prompt is needed).

use crate::auth::AuthManager;
use crate::tui::{AppCtx, AppMsg};

/// Clear the project's cached token (TUI counterpart of `hit logout`).
pub(super) fn logout(project_name: &str, ctx: &mut AppCtx) {
    let result = (|| {
        let project = crate::config::project(&ctx.services.config, project_name)?;
        if project.auth.is_none() {
            ctx.set_status(format!("'{project_name}' has no auth configured"));
            return Ok(());
        }
        let store = crate::auth::new_token_store(
            ctx.services.settings().token_store,
            ctx.services.paths.token_dir.clone(),
        )
        .map_err(crate::error::HitError::from)?;
        store
            .clear(project_name)
            .map_err(crate::error::HitError::from)?;
        ctx.set_status(format!("logged out of '{project_name}'"));
        Ok::<_, crate::error::HitError>(())
    })();
    if let Err(e) = result {
        ctx.show_error(e.to_string());
    }
}

/// Authenticate now (prompting through TUI modals as needed) and report
/// the result on the status line.
pub(super) fn login(project_name: String, ctx: &mut AppCtx) {
    let services = ctx.services.clone();
    let tx = ctx.tx.clone();
    let interactor = ctx.interactor();
    ctx.set_status(format!("logging in to '{project_name}'…"));
    tokio::spawn(async move {
        let result = async {
            let project = crate::config::project(&services.config, &project_name)
                .map_err(|e| e.to_string())?;
            let auth = AuthManager::for_project(
                &project_name,
                project,
                services.settings(),
                &services.paths,
                services.client.clone(),
                interactor,
                false,
            )
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!(
                    "project '{project_name}' has no auth configured — add a \
                     [projects.{project_name}.auth] block to projects.toml"
                )
            })?;
            auth.invalidate().await;
            auth.bearer().await.map_err(|e| e.to_string())?;
            Ok::<_, String>(auth.cached_expiry())
        }
        .await;
        let msg = match result {
            Ok(Some(exp)) => {
                let remaining = exp.saturating_sub(crate::auth::token_store::now_unix());
                AppMsg::Notify(format!(
                    "logged in to '{project_name}' (token expires in {remaining}s)"
                ))
            }
            Ok(None) => AppMsg::Notify(format!("logged in to '{project_name}'")),
            Err(message) => AppMsg::Error(message),
        };
        let _ = tx.send(msg);
    });
}
