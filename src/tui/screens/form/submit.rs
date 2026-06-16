//! Sending the request: serialize the form, spawn the execution, and push the
//! response screen. Also the seed JSON for the external editor.

use super::{Action, RequestForm, ResponseView};
use crate::http::RequestArgs;
use crate::tui::AppCtx;

impl RequestForm {
    pub(super) fn submit(&mut self, ctx: &mut AppCtx) -> Action {
        let serialized = match self.form.serialize() {
            Err(e) => {
                self.form.cursor = e.row;
                ctx.set_status(e.message);
                return Action::None;
            }
            Ok(s) => s,
        };

        let args = RequestArgs {
            path_params: serialized.path_params,
            query_params: serialized.query_params,
            headers: serialized.headers,
            body: serialized.body,
            no_auth: false,
        };
        let seq = ctx.spawn_request(self.bundle.project.clone(), self.endpoint.clone(), args);

        Action::Push(Box::new(ResponseView::loading(
            seq,
            self.bundle.clone(),
            self.endpoint.method.clone(),
            self.endpoint.path.clone(),
        )))
    }

    /// Seed JSON for external editing: current body, leniently serialized.
    pub(super) fn editor_seed(&self) -> String {
        let body = self.form.body_for_editing();
        serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string())
    }
}
