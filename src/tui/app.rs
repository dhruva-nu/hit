//! Application context shared across screens: services, async spawning,
//! modals, and the auth interactor that surfaces prompts as TUI modals.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::http::ApiResponse;
use crate::spec::SpecOrigin;
use crate::{AppServices, config, model::ApiSpec, spec};

/// A loaded, shareable spec bundle.
pub struct SpecBundle {
    pub project: String,
    pub spec: ApiSpec,
    pub origin: SpecOrigin,
}

/// Results of async work, sent back into the event loop.
pub enum AppMsg {
    SpecLoaded {
        project: String,
        result: Result<Arc<SpecBundle>, String>,
    },
    Response {
        request_seq: u64,
        result: Result<ApiResponse, String>,
    },
    /// An auth task needs a credential from the user; the answer goes back
    /// through `respond` (Err = user cancelled).
    Prompt {
        label: String,
        secret: bool,
        respond: std::sync::mpsc::Sender<Result<String, crate::error::AuthError>>,
    },
    /// Transient status-line text from a background task (e.g. OAuth URL).
    Notify(String),
    Error(String),
}

/// Shared context handed to screens: services, async spawning, modals.
pub struct AppCtx {
    pub services: Arc<AppServices>,
    pub tx: mpsc::UnboundedSender<AppMsg>,
    pub specs: HashMap<String, Arc<SpecBundle>>,
    pub modal: Option<Modal>,
    pub status: Option<String>,
    /// Monotonic id matching in-flight requests to Response messages.
    pub request_seq: u64,
    /// Animation frame counter (advanced by the tick timer).
    pub frame: u64,
}

impl AppCtx {
    pub fn show_error(&mut self, message: impl Into<String>) {
        self.modal = Some(Modal::Info {
            title: "error".into(),
            body: message.into(),
        });
    }

    /// Interactor that resolves prompts through TUI modals — used by every
    /// auth flow started from inside the TUI.
    pub fn interactor(&self) -> Arc<TuiInteractor> {
        Arc::new(TuiInteractor {
            tx: self.tx.clone(),
        })
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    /// Kick off a spec load for a project; result arrives as `SpecLoaded`.
    pub fn load_spec(&mut self, project_name: &str) {
        let services = self.services.clone();
        let tx = self.tx.clone();
        let name = project_name.to_string();
        tokio::spawn(async move {
            let result = match config::project(&services.config, &name) {
                Ok(project) => spec::load(
                    &services.client,
                    &name,
                    project,
                    services.settings(),
                    &services.paths.spec_cache_dir,
                    false,
                )
                .await
                .map(|loaded| {
                    Arc::new(SpecBundle {
                        project: name.clone(),
                        spec: loaded.spec,
                        origin: loaded.origin,
                    })
                })
                .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(AppMsg::SpecLoaded {
                project: name,
                result,
            });
        });
    }
}

pub enum Modal {
    Info {
        title: String,
        body: String,
    },
    /// Credential input: typed text accumulates in `input` (rendered masked
    /// when `secret`); Enter sends it back to the waiting auth task.
    Prompt {
        label: String,
        secret: bool,
        input: String,
        respond: std::sync::mpsc::Sender<Result<String, crate::error::AuthError>>,
    },
}

/// Bridges background auth tasks to the UI thread: prompts surface as
/// modals, and the task blocks until the user answers.
pub struct TuiInteractor {
    tx: mpsc::UnboundedSender<AppMsg>,
}

impl crate::auth::Interactor for TuiInteractor {
    fn prompt_line(&self, label: &str) -> Result<String, crate::error::AuthError> {
        self.prompt(label, false)
    }

    fn prompt_secret(&self, label: &str) -> Result<String, crate::error::AuthError> {
        self.prompt(label, true)
    }

    fn notify(&self, message: &str) {
        let _ = self.tx.send(AppMsg::Notify(message.to_string()));
    }
}

impl TuiInteractor {
    fn prompt(&self, label: &str, secret: bool) -> Result<String, crate::error::AuthError> {
        let (respond, answer) = std::sync::mpsc::channel();
        self.tx
            .send(AppMsg::Prompt {
                label: label.to_string(),
                secret,
                respond,
            })
            .map_err(|_| crate::error::AuthError::Credential("TUI shut down".into()))?;
        // Called from a spawned auth task; park this worker thread without
        // starving the runtime.
        tokio::task::block_in_place(|| answer.recv())
            .map_err(|_| crate::error::AuthError::Credential("prompt abandoned".into()))?
    }
}
