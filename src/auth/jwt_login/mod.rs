//! Local auth: POST credentials to the project's login endpoint, cache the
//! returned JWT, and re-login shortly before its `exp`.

mod jwt;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use url::Url;

use self::jwt::decode_jwt_exp;
use super::{AuthProvider, Interactor, StoredToken, TokenStore, resolve_credential};
use crate::config::{JwtLoginConfig, LoginContentType};
use crate::error::AuthError;

pub struct JwtLoginProvider {
    project: String,
    base_url: Url,
    config: JwtLoginConfig,
    store: Box<dyn TokenStore>,
    client: reqwest::Client,
    interactor: Arc<dyn Interactor>,
}

impl JwtLoginProvider {
    pub fn new(
        project: String,
        base_url: Url,
        config: JwtLoginConfig,
        store: Box<dyn TokenStore>,
        client: reqwest::Client,
        interactor: Arc<dyn Interactor>,
    ) -> Self {
        Self {
            project,
            base_url,
            config,
            store,
            client,
            interactor,
        }
    }

    fn login_url(&self) -> String {
        format!(
            "{}{}",
            self.base_url.as_str().trim_end_matches('/'),
            self.config.login_path
        )
    }

    async fn login(&self) -> Result<StoredToken, AuthError> {
        let username = resolve_credential(
            &self.config.username,
            &format!("[{}] username", self.project),
            self.interactor.as_ref(),
            false,
        )?;
        let password = resolve_credential(
            &self.config.password,
            &format!("[{}] password", self.project),
            self.interactor.as_ref(),
            true,
        )?;

        let url = self.login_url();
        let request = match self.config.login_content_type {
            // FastAPI's OAuth2PasswordRequestForm convention.
            LoginContentType::Form => self.client.post(&url).form(&[
                ("username", username.as_str()),
                ("password", password.as_str()),
            ]),
            LoginContentType::Json => self.client.post(&url).json(&serde_json::json!({
                "username": username,
                "password": password,
            })),
        };

        let response = request.send().await.map_err(|e| AuthError::LoginFailed {
            url: url.clone(),
            message: e.to_string(),
        })?;
        let stored = self.parse_login_response(response, &url).await?;
        self.store.save(&self.project, &stored)?;
        tracing::info!(project = self.project, "logged in");
        Ok(stored)
    }

    /// Validate the HTTP response and pull the configured token out of its body.
    async fn parse_login_response(
        &self,
        response: reqwest::Response,
        url: &str,
    ) -> Result<StoredToken, AuthError> {
        let status = response.status();
        let body: Value = response.json().await.map_err(|e| AuthError::LoginFailed {
            url: url.to_string(),
            message: format!("non-JSON login response: {e}"),
        })?;
        if !status.is_success() {
            return Err(AuthError::LoginFailed {
                url: url.to_string(),
                message: format!("{status}: {body}"),
            });
        }

        let token = body
            .pointer(&self.config.token_json_pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| AuthError::TokenNotFound(self.config.token_json_pointer.clone()))?
            .to_string();

        Ok(StoredToken {
            expires_at_unix: decode_jwt_exp(&token),
            access_token: token,
            refresh_token: None,
            token_type: "Bearer".into(),
        })
    }
}

#[async_trait]
impl AuthProvider for JwtLoginProvider {
    async fn token(&self) -> Result<String, AuthError> {
        if let Some(stored) = self.store.load(&self.project)
            && stored.is_fresh(self.config.refresh_margin_secs)
        {
            return Ok(stored.access_token);
        }
        Ok(self.login().await?.access_token)
    }

    async fn invalidate(&self) {
        if let Err(e) = self.store.clear(&self.project) {
            tracing::warn!(project = self.project, error = %e, "failed to clear token");
        }
    }

    /// Headless only when credentials don't require a prompt.
    fn supports_headless(&self) -> bool {
        use crate::config::CredentialRef;
        let prompts = |c: &CredentialRef| matches!(c, CredentialRef::Prompt { .. });
        !prompts(&self.config.username) && !prompts(&self.config.password)
    }

    fn cached_expiry(&self) -> Option<u64> {
        self.store.load(&self.project)?.expires_at_unix
    }
}
