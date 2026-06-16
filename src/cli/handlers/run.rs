//! `hit run` handler.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::AppServices;
use crate::auth::{AuthManager, CliInteractor};
use crate::cli::RunArgs;
use crate::cli::handlers::load_spec;
use crate::cli::output::CommandOutput;
use crate::cli::parse::{parse_body, parse_header, parse_kv};
use crate::config;
use crate::error::{HitError, exit_code};
use crate::http::{ApiResponse, RequestArgs, RequestExecutor};
use crate::spec::adapter::adapter_for;

pub(crate) async fn run_cmd(
    args: RunArgs,
    no_cache: bool,
    services: &AppServices,
) -> Result<CommandOutput, HitError> {
    let project = config::project(&services.config, &args.project)?;
    let loaded = load_spec(&args.project, no_cache, services).await?;
    let endpoint = loaded.spec.find_endpoint(&args.endpoint)?;

    let request_args = build_request_args(&args)?;

    let auth = AuthManager::for_project(
        &args.project,
        project,
        services.settings(),
        &services.paths,
        services.client.clone(),
        Arc::new(CliInteractor),
        false,
    )?;

    let executor = RequestExecutor {
        client: &services.client,
        project,
        auth: auth.as_ref(),
    };
    let response = executor.execute(endpoint, &request_args).await?;

    let human = render_human(project, &response);
    let exit = if args.allow_error {
        exit_code::OK
    } else {
        response.exit_code()
    };
    let data = serde_json::to_value(&response).map_err(|e| HitError::Other(e.to_string()))?;
    Ok(CommandOutput::ok(data, human).with_exit(exit))
}

/// Parse the CLI's `name=value` / `Key: Value` strings into request inputs.
fn build_request_args(args: &RunArgs) -> Result<RequestArgs, HitError> {
    let mut path_params = BTreeMap::new();
    for kv in &args.path_params {
        let (k, v) = parse_kv(kv)?;
        path_params.insert(k, v);
    }
    let mut query_params = Vec::new();
    for kv in &args.query {
        query_params.push(parse_kv(kv)?);
    }
    let mut headers = Vec::new();
    for header in &args.headers {
        headers.push(parse_header(header)?);
    }
    let body = args.body.as_deref().map(parse_body).transpose()?;
    Ok(RequestArgs::from_parts(
        path_params,
        query_params,
        headers,
        body,
        args.no_auth,
    ))
}

fn render_human(project: &config::ProjectConfig, response: &ApiResponse) -> String {
    let mut human = format!(
        "{} {} -> {} ({} ms)",
        response.method, response.url, response.status, response.latency_ms
    );
    if !response.is_success()
        && let Some(lines) =
            adapter_for(project.framework).render_error_lines(response.status, &response.body)
    {
        for line in &lines {
            human.push_str(&format!("\n  ! {line}"));
        }
    }
    let body_pretty = if response.body_is_json {
        serde_json::to_string_pretty(&response.body).unwrap_or_default()
    } else {
        response.body.as_str().unwrap_or("").to_string()
    };
    if !body_pretty.is_empty() {
        human.push_str("\n\n");
        human.push_str(&body_pretty);
    }
    human
}
