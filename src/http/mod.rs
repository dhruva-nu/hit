//! Request execution: build the URL from the endpoint template, attach auth,
//! send, and (once) retry on 401 after invalidating cached credentials.

mod body;
mod response;

pub use response::ApiResponse;

use std::collections::BTreeMap;

use serde_json::Value;

use crate::auth::AuthManager;
use crate::config::ProjectConfig;
use crate::error::{HitError, RequestError};
use crate::model::Endpoint;

use body::{attach_body, fill_path};

/// User-supplied request inputs, the same shape for CLI, MCP, and TUI.
#[derive(Debug, Clone, Default)]
pub struct RequestArgs {
    pub path_params: BTreeMap<String, String>,
    pub query_params: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
    pub no_auth: bool,
}

impl RequestArgs {
    /// Assemble request inputs from already-collected parts. Shared by the CLI
    /// (after parsing `name=value` strings) and MCP (after unwrapping Options)
    /// so both produce identical request arguments.
    pub fn from_parts(
        path_params: BTreeMap<String, String>,
        query_params: Vec<(String, String)>,
        headers: Vec<(String, String)>,
        body: Option<Value>,
        no_auth: bool,
    ) -> Self {
        Self {
            path_params,
            query_params,
            headers,
            body,
            no_auth,
        }
    }
}

pub struct RequestExecutor<'a> {
    pub client: &'a reqwest::Client,
    pub project: &'a ProjectConfig,
    pub auth: Option<&'a AuthManager>,
}

impl RequestExecutor<'_> {
    pub async fn execute(
        &self,
        endpoint: &Endpoint,
        args: &RequestArgs,
    ) -> Result<ApiResponse, HitError> {
        let url = self.build_url(endpoint, args)?;
        let use_auth = !args.no_auth && self.auth.is_some();

        let mut bearer = match (use_auth, self.auth) {
            (true, Some(auth)) => auth.bearer().await.map(Some)?,
            _ => None,
        };

        let started = std::time::Instant::now();
        let mut response = self.send(endpoint, args, &url, bearer.as_deref()).await?;

        // One reactive retry: cached token may have just expired.
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            && let (true, Some(auth)) = (use_auth, self.auth)
        {
            tracing::info!(url, "got 401; invalidating cached token and retrying once");
            auth.invalidate().await;
            bearer = Some(auth.bearer().await?);
            response = self.send(endpoint, args, &url, bearer.as_deref()).await?;
        }

        ApiResponse::from_reqwest(endpoint.method.clone(), url, response, started.elapsed()).await
    }

    fn build_url(&self, endpoint: &Endpoint, args: &RequestArgs) -> Result<String, RequestError> {
        let path = fill_path(&endpoint.path, &args.path_params)?;
        Ok(format!(
            "{}{}",
            self.project.base_url.as_str().trim_end_matches('/'),
            path
        ))
    }

    fn attach_headers(
        &self,
        mut request: reqwest::RequestBuilder,
        args: &RequestArgs,
        bearer: Option<&str>,
    ) -> reqwest::RequestBuilder {
        for (name, value) in &self.project.default_headers {
            request = request.header(name, value);
        }
        for (name, value) in &args.headers {
            request = request.header(name, value);
        }
        if let Some(token) = bearer {
            request = request.bearer_auth(token);
        }
        request
    }

    async fn send(
        &self,
        endpoint: &Endpoint,
        args: &RequestArgs,
        url: &str,
        bearer: Option<&str>,
    ) -> Result<reqwest::Response, HitError> {
        let method: reqwest::Method = endpoint
            .method
            .parse()
            .map_err(|_| RequestError::InvalidHeader(endpoint.method.clone()))?;
        let mut request = self.client.request(method, url);

        request = self.attach_headers(request, args, bearer);
        if !args.query_params.is_empty() {
            request = request.query(&args.query_params);
        }
        if let Some(body) = &args.body {
            request = attach_body(request, endpoint, body)?;
        }

        request.send().await.map_err(|e| {
            if e.is_timeout() {
                HitError::Request(RequestError::Timeout(0))
            } else {
                HitError::Request(RequestError::Network {
                    url: url.to_string(),
                    message: e.to_string(),
                })
            }
        })
    }
}
