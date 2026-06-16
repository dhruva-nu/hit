//! `hit config` handlers.

use std::path::PathBuf;

use serde_json::json;

use crate::AppServices;
use crate::cli::output::CommandOutput;
use crate::config;
use crate::error::HitError;

pub(crate) fn config_check_cmd(
    config_override: &Option<PathBuf>,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    // Config was already loaded+validated at startup; re-validate explicitly
    // so the command works as a standalone health check.
    config::validate(&services.config)?;
    let path = config_override
        .clone()
        .unwrap_or_else(|| services.paths.config_file.clone());
    let human = format!(
        "{} OK — {} project(s)",
        path.display(),
        services.config.projects.len()
    );
    Ok(CommandOutput::ok(
        json!({"projects": services.config.projects.len()}),
        human,
    ))
}
