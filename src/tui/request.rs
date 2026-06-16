//! Spawning an HTTP request from the UI: resolve auth, execute the endpoint,
//! and deliver the outcome back to the event loop as an `AppMsg::Response`.

use crate::auth::AuthManager;
use crate::config;
use crate::http::{RequestArgs, RequestExecutor};
use crate::model::Endpoint;

use super::{AppCtx, AppMsg};

impl AppCtx {
    /// Execute `endpoint` with `args` on the runtime. The result arrives as an
    /// [`AppMsg::Response`] tagged with the returned sequence id, which the
    /// caller stores to match the reply to the screen that fired it.
    pub(crate) fn spawn_request(
        &mut self,
        project_name: String,
        endpoint: Endpoint,
        args: RequestArgs,
    ) -> u64 {
        self.request_seq += 1;
        let seq = self.request_seq;
        let services = self.services.clone();
        let tx = self.tx.clone();
        let interactor = self.interactor();

        tokio::spawn(async move {
            let result = async {
                let project =
                    config::project(&services.config, &project_name).map_err(|e| e.to_string())?;
                let auth = AuthManager::for_project(
                    &project_name,
                    project,
                    services.settings(),
                    &services.paths,
                    services.client.clone(),
                    interactor,
                    false,
                )
                .map_err(|e| e.to_string())?;
                let executor = RequestExecutor {
                    client: &services.client,
                    project,
                    auth: auth.as_ref(),
                };
                executor
                    .execute(&endpoint, &args)
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;
            let _ = tx.send(AppMsg::Response {
                request_seq: seq,
                result,
            });
        });

        seq
    }
}
