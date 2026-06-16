//! Validation of `ProjectsConfig` beyond what serde enforces structurally.

use crate::error::ConfigError;

use super::schema::{AuthConfig, ProjectConfig, ProjectsConfig};

/// Validation beyond what serde enforces structurally.
pub fn validate(config: &ProjectsConfig) -> Result<(), ConfigError> {
    for (name, project) in &config.projects {
        validate_project(name, project)?;
    }
    Ok(())
}

/// Per-project checks: name shape, base URL scheme, spec file presence, and
/// any auth-specific rules.
fn validate_project(name: &str, project: &ProjectConfig) -> Result<(), ConfigError> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || "-_".contains(c))
    {
        return Err(ConfigError::Invalid {
            field: format!("projects.{name}"),
            message: "project names must be alphanumeric plus '-' or '_'".into(),
        });
    }
    if !matches!(project.base_url.scheme(), "http" | "https") {
        return Err(ConfigError::Invalid {
            field: format!("projects.{name}.base_url"),
            message: format!("unsupported scheme '{}'", project.base_url.scheme()),
        });
    }
    if let Some(spec_file) = &project.spec_file
        && !spec_file.exists()
    {
        // Warn, don't fail: the fallback only matters when the server is down.
        tracing::warn!(
            project = name,
            spec_file = %spec_file.display(),
            "configured spec_file does not exist"
        );
    }
    validate_auth(name, project)
}

/// Auth-specific checks (currently JWT login path / token pointer shape).
fn validate_auth(name: &str, project: &ProjectConfig) -> Result<(), ConfigError> {
    if let Some(AuthConfig::JwtLogin(jwt)) = &project.auth {
        if !jwt.login_path.starts_with('/') {
            return Err(ConfigError::Invalid {
                field: format!("projects.{name}.auth.login_path"),
                message: "login_path must start with '/'".into(),
            });
        }
        if !jwt.token_json_pointer.starts_with('/') {
            return Err(ConfigError::Invalid {
                field: format!("projects.{name}.auth.token_json_pointer"),
                message: "JSON pointers must start with '/'".into(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, CredentialRef};

    #[test]
    fn parses_full_example() {
        let toml_src = r#"
            [settings]
            spec_cache_ttl_secs = 60

            [projects.billing]
            base_url = "http://localhost:8000"
            default_headers = { "X-Tenant" = "dev" }

            [projects.billing.auth]
            type = "jwt_login"
            login_path = "/auth/login"
            username = { env = "BILLING_USER" }
            password = { prompt = true }

            [projects.crm]
            base_url = "https://crm.example.com"

            [projects.crm.auth]
            type = "oauth2_pkce"
            auth_url = "https://idp.example.com/authorize"
            token_url = "https://idp.example.com/oauth/token"
            client_id = "hitpoint"
            scopes = ["openid"]
        "#;
        let config: ProjectsConfig = toml::from_str(toml_src).unwrap();
        validate(&config).unwrap();
        assert_eq!(config.settings.spec_cache_ttl_secs, 60);
        assert_eq!(config.settings.timeout_secs, 30); // default survives partial [settings]
        let billing = &config.projects["billing"];
        match billing.auth.as_ref().unwrap() {
            AuthConfig::JwtLogin(jwt) => {
                assert!(matches!(jwt.username, CredentialRef::Env { .. }));
                assert!(matches!(jwt.password, CredentialRef::Prompt { .. }));
                assert_eq!(jwt.token_json_pointer, "/access_token");
            }
            other => panic!("expected jwt_login, got {}", other.type_name()),
        }
        assert!(matches!(
            config.projects["crm"].auth,
            Some(AuthConfig::Oauth2Pkce(_))
        ));
    }

    #[test]
    fn rejects_bad_login_path() {
        let toml_src = r#"
            [projects.x]
            base_url = "http://localhost:1"
            [projects.x.auth]
            type = "jwt_login"
            login_path = "auth/login"
            username = { value = "u" }
            password = { value = "p" }
        "#;
        let config: ProjectsConfig = toml::from_str(toml_src).unwrap();
        assert!(validate(&config).is_err());
    }
}
