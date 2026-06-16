//! `hit projects` handlers.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::AppServices;
use crate::cli::ProjectsCmd;
use crate::cli::output::CommandOutput;
use crate::cli::parse::parse_header;
use crate::config::{self, ProjectConfig};
use crate::error::{ConfigError, HitError};

pub(crate) async fn projects_cmd(
    cmd: ProjectsCmd,
    services: &mut AppServices,
) -> Result<CommandOutput, HitError> {
    match cmd {
        ProjectsCmd::List => Ok(projects_list(services)),
        ProjectsCmd::Add {
            name,
            base_url,
            spec_file,
            headers,
        } => projects_add(name, base_url, spec_file, headers, services),
        ProjectsCmd::Remove { name } => projects_remove(name, services),
    }
}

fn projects_list(services: &AppServices) -> CommandOutput {
    let rows: Vec<Value> = services
        .config
        .projects
        .iter()
        .map(|(name, p)| {
            json!({
                "name": name,
                "base_url": p.base_url.as_str(),
                "auth_type": p.auth.as_ref().map(|a| a.type_name()),
                "spec_file": p.spec_file.as_ref().map(|f| f.display().to_string()),
            })
        })
        .collect();
    let human = if rows.is_empty() {
        "no projects registered — add one with: hit projects add <name> --base-url <url>"
            .to_string()
    } else {
        services
            .config
            .projects
            .iter()
            .map(|(name, p)| {
                format!(
                    "{name:<20} {}  [auth: {}]",
                    p.base_url,
                    p.auth.as_ref().map_or("none", |a| a.type_name())
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    CommandOutput::ok(json!(rows), human)
}

fn projects_add(
    name: String,
    base_url: url::Url,
    spec_file: Option<std::path::PathBuf>,
    headers: Vec<String>,
    services: &mut AppServices,
) -> Result<CommandOutput, HitError> {
    if services.config.projects.contains_key(&name) {
        return Err(ConfigError::DuplicateProject(name).into());
    }
    let mut default_headers = BTreeMap::new();
    for header in &headers {
        let (key, value) = parse_header(header)?;
        default_headers.insert(key, value);
    }
    services.config.projects.insert(
        name.clone(),
        ProjectConfig {
            base_url,
            spec_file,
            default_headers,
            auth: None,
            framework: Default::default(),
        },
    );
    config::save(&services.paths, &services.config)?;
    Ok(CommandOutput::ok(
        json!({"added": name}),
        format!(
            "added project '{name}'. Configure auth by editing {}",
            services.paths.config_file.display()
        ),
    ))
}

fn projects_remove(name: String, services: &mut AppServices) -> Result<CommandOutput, HitError> {
    if services.config.projects.remove(&name).is_none() {
        return Err(ConfigError::UnknownProject {
            name,
            available: services.config.projects.keys().cloned().collect(),
        }
        .into());
    }
    config::save(&services.paths, &services.config)?;
    // Best-effort cleanup of cached state.
    let _ = std::fs::remove_file(services.paths.spec_cache_dir.join(format!("{name}.json")));
    let _ = std::fs::remove_file(services.paths.token_dir.join(format!("{name}.json")));
    Ok(CommandOutput::ok(
        json!({"removed": name}),
        format!("removed project '{name}'"),
    ))
}
