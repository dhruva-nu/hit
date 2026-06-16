//! Loading, validating, and saving `projects.toml`, plus XDG path resolution.

mod schema;
mod validate;

pub use schema::*;
pub use validate::validate;

use std::path::{Path, PathBuf};

use crate::error::ConfigError;

/// Resolved filesystem locations for config, cache, and data.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub spec_cache_dir: PathBuf,
    pub token_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl Paths {
    /// Standard XDG locations, or everything rooted next to an explicit config file.
    pub fn resolve(config_override: Option<&Path>) -> Result<Self, ConfigError> {
        if let Some(file) = config_override {
            let root = file.parent().unwrap_or(Path::new(".")).to_path_buf();
            return Ok(Self {
                config_file: file.to_path_buf(),
                spec_cache_dir: root.join("cache/specs"),
                token_dir: root.join("tokens"),
                log_dir: root.join("logs"),
            });
        }
        let dirs = directories::ProjectDirs::from("", "", "hitpoint").ok_or_else(|| {
            ConfigError::Invalid {
                field: "paths".into(),
                message: "could not determine a home directory".into(),
            }
        })?;
        Ok(Self {
            config_file: dirs.config_dir().join("projects.toml"),
            spec_cache_dir: dirs.cache_dir().join("specs"),
            token_dir: dirs.data_dir().join("tokens"),
            log_dir: dirs.data_dir().join("logs"),
        })
    }
}

/// Load and validate the config. A missing file is an empty config, not an error,
/// so `hit projects add` works on first run.
pub fn load(paths: &Paths) -> Result<ProjectsConfig, ConfigError> {
    let raw = match std::fs::read_to_string(&paths.config_file) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProjectsConfig::default());
        }
        Err(e) => {
            return Err(ConfigError::Io {
                path: paths.config_file.display().to_string(),
                source: e,
            });
        }
    };
    let config: ProjectsConfig = toml::from_str(&raw)
        .map_err(|e| ConfigError::Parse(format!("{}: {e}", paths.config_file.display())))?;
    validate(&config)?;
    Ok(config)
}

/// Atomically persist the config (temp file + rename).
pub fn save(paths: &Paths, config: &ProjectsConfig) -> Result<(), ConfigError> {
    validate(config)?;
    let serialized = toml::to_string_pretty(config)
        .map_err(|e| ConfigError::Parse(format!("serialize: {e}")))?;
    let dir = paths
        .config_file
        .parent()
        .ok_or_else(|| ConfigError::Invalid {
            field: "config path".into(),
            message: "config file has no parent directory".into(),
        })?;
    std::fs::create_dir_all(dir).map_err(|e| ConfigError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;
    let tmp = paths.config_file.with_extension("toml.tmp");
    std::fs::write(&tmp, serialized).map_err(|e| ConfigError::Io {
        path: tmp.display().to_string(),
        source: e,
    })?;
    std::fs::rename(&tmp, &paths.config_file).map_err(|e| ConfigError::Io {
        path: paths.config_file.display().to_string(),
        source: e,
    })
}

/// Fetch one project or fail with suggestions.
pub fn project<'a>(
    config: &'a ProjectsConfig,
    name: &str,
) -> Result<&'a ProjectConfig, ConfigError> {
    config
        .projects
        .get(name)
        .ok_or_else(|| ConfigError::UnknownProject {
            name: name.to_string(),
            available: config.projects.keys().cloned().collect(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_project_lists_alternatives() {
        let config: ProjectsConfig =
            toml::from_str("[projects.alpha]\nbase_url = \"http://x\"").unwrap();
        let err = project(&config, "beta").unwrap_err();
        assert!(err.to_string().contains("alpha"));
    }
}
