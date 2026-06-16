//! Headless command handlers, grouped by command family.

mod auth;
mod config;
mod endpoints;
mod projects;
mod run;
mod spec;

use std::path::PathBuf;

use serde_json::Value;

use crate::cli::output::CommandOutput;
use crate::cli::{Cli, Command, ConfigCmd, SpecCmd};
use crate::error::HitError;
use crate::{AppServices, spec as spec_mod};

use clap::CommandFactory;

pub(crate) async fn dispatch(
    command: Command,
    config_override: &Option<PathBuf>,
    no_cache: bool,
    mut services: AppServices,
) -> Result<CommandOutput, HitError> {
    match command {
        Command::Projects { cmd } => projects::projects_cmd(cmd, &mut services).await,
        Command::Tags { project } => spec::tags_cmd(&project, no_cache, &services).await,
        Command::Endpoints {
            project,
            tag,
            search,
        } => {
            endpoints::endpoints_cmd(
                &project,
                tag.as_deref(),
                search.as_deref(),
                no_cache,
                &services,
            )
            .await
        }
        Command::Template { project, endpoint } => {
            spec::template_cmd(&project, &endpoint, no_cache, &services).await
        }
        Command::Run(args) => run::run_cmd(args, no_cache, &services).await,
        Command::Login { project } => auth::login_cmd(&project, &services).await,
        Command::Logout { project } => auth::logout_cmd(&project, &services),
        Command::Spec { cmd } => match cmd {
            SpecCmd::Refresh { project } => spec::refresh_cmd(&project, &services).await,
        },
        Command::Config { cmd } => match cmd {
            ConfigCmd::Check => config::config_check_cmd(config_override, &services),
        },
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "hit", &mut std::io::stdout());
            Ok(CommandOutput::ok(Value::Null, ""))
        }
        Command::Mcp | Command::Tui { .. } => {
            unreachable!("mcp/tui are dispatched from main")
        }
    }
}

/// Load a project's OpenAPI spec (cached unless `no_cache`).
pub(crate) async fn load_spec(
    project_name: &str,
    no_cache: bool,
    services: &AppServices,
) -> Result<spec_mod::LoadedSpec, HitError> {
    let project = crate::config::project(&services.config, project_name)?;
    spec_mod::load(
        &services.client,
        project_name,
        project,
        services.settings(),
        &services.paths.spec_cache_dir,
        no_cache,
    )
    .await
    .map_err(Into::into)
}
