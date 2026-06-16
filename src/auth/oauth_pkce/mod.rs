//! 3rd-party auth: OAuth2 authorization-code flow with PKCE. Opens the
//! system browser, captures the code on a loopback listener, exchanges and
//! refreshes tokens.
//!
//! MCP/headless contexts never reach the browser path: with no usable cached
//! or refreshable token they fail with an instruction to run `hit login`.

mod browser;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, RedirectUrl, RefreshToken, TokenResponse, TokenUrl};

use super::{AuthProvider, Interactor, StoredToken, TokenStore, token_store::now_unix};
use crate::config::OAuth2PkceConfig;
use crate::error::AuthError;

/// How long we wait for the browser to hit the loopback callback.
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
/// Refresh this many seconds before expiry.
const REFRESH_MARGIN_SECS: u64 = 60;

/// Concrete `BasicClient` type with the endpoints this flow sets.
type OAuthClient = BasicClient<
    oauth2::EndpointSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointSet,
>;

pub struct OAuth2PkceProvider {
    project: String,
    config: OAuth2PkceConfig,
    store: Box<dyn TokenStore>,
    interactor: Arc<dyn Interactor>,
    /// True in MCP/non-interactive contexts: never launch a browser.
    headless: bool,
}

impl OAuth2PkceProvider {
    pub fn new(
        project: String,
        config: OAuth2PkceConfig,
        store: Box<dyn TokenStore>,
        _client: reqwest::Client,
        interactor: Arc<dyn Interactor>,
        headless: bool,
    ) -> Self {
        // Note: the oauth2 crate carries its own reqwest (with redirects
        // disabled, as the RFC requires); the shared app client isn't reused.
        Self {
            project,
            config,
            store,
            interactor,
            headless,
        }
    }

    fn oauth_client(&self, redirect_uri: Option<String>) -> Result<OAuthClient, AuthError> {
        let mut client = BasicClient::new(ClientId::new(self.config.client_id.clone()))
            .set_auth_uri(
                AuthUrl::new(self.config.auth_url.to_string())
                    .map_err(|e| AuthError::OAuth(format!("auth_url: {e}")))?,
            )
            .set_token_uri(
                TokenUrl::new(self.config.token_url.to_string())
                    .map_err(|e| AuthError::OAuth(format!("token_url: {e}")))?,
            );
        if let Some(uri) = redirect_uri {
            client = client.set_redirect_uri(
                RedirectUrl::new(uri).map_err(|e| AuthError::OAuth(format!("redirect: {e}")))?,
            );
        }
        Ok(client)
    }

    fn store_response(
        &self,
        response: &impl TokenResponse,
        previous_refresh: Option<String>,
    ) -> Result<StoredToken, AuthError> {
        let stored = StoredToken {
            access_token: response.access_token().secret().clone(),
            refresh_token: response
                .refresh_token()
                .map(|t| t.secret().clone())
                .or(previous_refresh),
            expires_at_unix: response.expires_in().map(|d| now_unix() + d.as_secs()),
            token_type: "Bearer".into(),
        };
        self.store.save(&self.project, &stored)?;
        Ok(stored)
    }

    async fn try_refresh(&self, refresh_token: &str) -> Result<StoredToken, AuthError> {
        tracing::info!(project = self.project, "refreshing OAuth token");
        let client = self.oauth_client(None)?;
        let response = client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
            .request_async(&browser::http_client()?)
            .await
            .map_err(|e| AuthError::OAuth(format!("refresh failed: {e}")))?;
        self.store_response(&response, Some(refresh_token.to_string()))
    }
}

#[async_trait]
impl AuthProvider for OAuth2PkceProvider {
    async fn token(&self) -> Result<String, AuthError> {
        let stored = self.store.load(&self.project);

        if let Some(token) = &stored
            && token.is_fresh(REFRESH_MARGIN_SECS)
        {
            return Ok(token.access_token.clone());
        }

        if let Some(refresh_token) = stored.as_ref().and_then(|t| t.refresh_token.clone()) {
            match self.try_refresh(&refresh_token).await {
                Ok(token) => return Ok(token.access_token),
                Err(e) => tracing::warn!(
                    project = self.project,
                    error = %e,
                    "token refresh failed; falling back to full re-auth"
                ),
            }
        }

        if self.headless {
            return Err(AuthError::InteractionRequired(format!(
                "project '{}' uses browser-based OAuth — run `hit login {}` in a terminal",
                self.project, self.project
            )));
        }

        Ok(self.authorization_code_flow().await?.access_token)
    }

    async fn invalidate(&self) {
        // Keep the refresh token: a 401 usually means the access token aged
        // out; the next token() call will refresh, or fully re-auth if that
        // fails too.
        if let Some(mut stored) = self.store.load(&self.project)
            && stored.refresh_token.is_some()
        {
            stored.expires_at_unix = Some(0);
            let _ = self.store.save(&self.project, &stored);
            return;
        }
        if let Err(e) = self.store.clear(&self.project) {
            tracing::warn!(project = self.project, error = %e, "failed to clear token");
        }
    }

    fn supports_headless(&self) -> bool {
        false
    }

    fn cached_expiry(&self) -> Option<u64> {
        self.store.load(&self.project)?.expires_at_unix
    }
}
