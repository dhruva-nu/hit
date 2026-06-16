//! Browser-driven half of the PKCE flow: loopback listener, system browser,
//! and the tiny HTTP responder for the callback tab.

use oauth2::{AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

use super::{CALLBACK_TIMEOUT, OAuth2PkceProvider, OAuthClient, StoredToken};
use crate::error::AuthError;

impl OAuth2PkceProvider {
    /// Full browser flow: loopback listener first, then browser.
    pub(super) async fn authorization_code_flow(&self) -> Result<StoredToken, AuthError> {
        let (listener, redirect_uri) = Self::bind_callback(self.config.redirect_port).await?;
        let client = self.oauth_client(Some(redirect_uri))?;
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (auth_url, csrf_state) = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(self.config.scopes.iter().map(|s| Scope::new(s.clone())))
            .set_pkce_challenge(pkce_challenge)
            .url();

        self.launch_browser(auth_url.as_str());

        let (code, state) = tokio::time::timeout(CALLBACK_TIMEOUT, accept_callback(&listener))
            .await
            .map_err(|_| {
                AuthError::OAuth(format!(
                    "timed out after {}s waiting for the OAuth callback",
                    CALLBACK_TIMEOUT.as_secs()
                ))
            })??;

        if state != *csrf_state.secret() {
            return Err(AuthError::OAuth(
                "state mismatch in OAuth callback — possible CSRF; aborting".into(),
            ));
        }
        self.exchange_code(&client, code, pkce_verifier).await
    }

    /// Bind a loopback listener and derive its callback redirect URI.
    async fn bind_callback(redirect_port: u16) -> Result<(TcpListener, String), AuthError> {
        let listener = TcpListener::bind(("127.0.0.1", redirect_port))
            .await
            .map_err(|e| AuthError::OAuth(format!("binding callback listener: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| AuthError::OAuth(e.to_string()))?
            .port();
        Ok((listener, format!("http://127.0.0.1:{port}/callback")))
    }

    /// Notify the user and open the system browser (unless suppressed).
    fn launch_browser(&self, auth_url: &str) {
        self.interactor.notify(&format!(
            "Opening browser for OAuth login. If nothing happens, open:\n  {auth_url}"
        ));
        // HITPOINT_NO_BROWSER: print the URL only (SSH sessions, tests).
        if std::env::var_os("HITPOINT_NO_BROWSER").is_none()
            && let Err(e) = open::that(auth_url)
        {
            tracing::warn!(error = %e, "failed to launch browser; user must open the URL manually");
        }
    }

    /// Exchange the authorization code for tokens and persist them.
    async fn exchange_code(
        &self,
        client: &OAuthClient,
        code: String,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<StoredToken, AuthError> {
        let response = client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client()?)
            .await
            .map_err(|e| AuthError::OAuth(format!("code exchange failed: {e}")))?;
        tracing::info!(project = self.project, "OAuth login complete");
        self.store_response(&response, None)
    }
}

pub(super) fn http_client() -> Result<oauth2::reqwest::Client, AuthError> {
    // Redirects must stay disabled on the token endpoint (SSRF hardening).
    oauth2::reqwest::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AuthError::OAuth(format!("http client: {e}")))
}

/// Accept one connection and pull `code` and `state` out of the request line.
async fn accept_callback(listener: &TcpListener) -> Result<(String, String), AuthError> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| AuthError::OAuth(format!("callback accept: {e}")))?;

        // Malformed line: keep listening until the timeout fires.
        let Some(path) = read_request_path(&mut stream).await? else {
            continue;
        };
        // Browsers also ask for /favicon.ico etc.
        if !path.starts_with("/callback") {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
            continue;
        }
        if let Some(result) = parse_callback(&mut stream, &path).await? {
            return Ok(result);
        }
    }
}

/// Read the request and return its request-target path, if any.
async fn read_request_path(stream: &mut TcpStream) -> Result<Option<String>, AuthError> {
    let mut buf = vec![0u8; 8192];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| AuthError::OAuth(format!("callback read: {e}")))?;
    let request = String::from_utf8_lossy(&buf[..n]);
    Ok(request.split_whitespace().nth(1).map(|p| p.to_string()))
}

/// Parse a `/callback` request: `Ok(Some(..))` on success, `Ok(None)` to keep
/// listening, `Err` on an authorization-server error.
async fn parse_callback(
    stream: &mut TcpStream,
    path: &str,
) -> Result<Option<(String, String)>, AuthError> {
    let parsed = Url::parse(&format!("http://localhost{path}"))
        .map_err(|e| AuthError::OAuth(format!("callback URL: {e}")))?;
    let get = |key: &str| {
        parsed
            .query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };

    if let Some(error) = get("error") {
        let description = get("error_description").unwrap_or_default();
        let _ = respond_html(stream, "Login failed — you can close this tab.").await;
        return Err(AuthError::OAuth(format!(
            "authorization server returned '{error}': {description}"
        )));
    }

    match (get("code"), get("state")) {
        (Some(code), Some(state)) => {
            let _ =
                respond_html(stream, "hitpoint: login complete — you can close this tab.").await;
            Ok(Some((code, state)))
        }
        _ => {
            let _ = respond_html(stream, "Missing code/state in callback.").await;
            Ok(None) // keep listening
        }
    }
}

async fn respond_html(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}
