//! The right-hand reference panel: pick a related GET endpoint, fire it, and
//! show its response inline. Also the no-input request preview.

use super::RequestForm;
use crate::http::{ApiResponse, RequestArgs};
use crate::model::{Endpoint, ParamLocation};
use crate::tui::{AppCtx, SpecBundle};

pub(super) enum PanelState {
    Picker { selected: usize },
    Loading,
    Done { response: ApiResponse, scroll: u16 },
    Error(String),
}

pub(super) struct RightPanel {
    /// Indices into `bundle.spec.endpoints` where `method == "GET"`.
    pub(super) get_endpoints: Vec<usize>,
    /// Sequence id of the in-flight or most recent reference GET.
    pub(super) request_seq: Option<u64>,
    /// Whether the fired GET had a `limit` / `per_page` / `page_size` injected.
    pub(super) limit_injected: bool,
    pub(super) state: PanelState,
}

impl RightPanel {
    /// Build the panel for a non-GET endpoint, preferring GETs that share a
    /// tag; falls back to all GETs. None for GET endpoints or when no GETs.
    pub(super) fn for_endpoint(bundle: &SpecBundle, endpoint: &Endpoint) -> Option<Self> {
        if endpoint.method == "GET" {
            return None;
        }
        let current_tags = &endpoint.tags;
        let get_endpoints: Vec<usize> = bundle
            .spec
            .endpoints
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.method == "GET"
                    && (current_tags.is_empty() || e.tags.iter().any(|t| current_tags.contains(t)))
            })
            .map(|(i, _)| i)
            .collect();
        // If tag filtering yields nothing, fall back to all GETs.
        let get_endpoints = if get_endpoints.is_empty() {
            bundle
                .spec
                .endpoints
                .iter()
                .enumerate()
                .filter(|(_, e)| e.method == "GET")
                .map(|(i, _)| i)
                .collect()
        } else {
            get_endpoints
        };
        if get_endpoints.is_empty() {
            None
        } else {
            Some(RightPanel {
                get_endpoints,
                request_seq: None,
                limit_injected: false,
                state: PanelState::Picker { selected: 0 },
            })
        }
    }
}

impl RequestForm {
    /// Fire the GET endpoint currently selected in the picker.
    pub(super) fn fire_reference_get(&mut self, ctx: &mut AppCtx) {
        let panel = match &mut self.right_panel {
            Some(p) => p,
            None => return,
        };
        let selected_idx = match &panel.state {
            PanelState::Picker { selected } => panel.get_endpoints[*selected],
            _ => return,
        };

        let endpoint = self.bundle.spec.endpoints[selected_idx].clone();

        let limit_param = endpoint.params_in(ParamLocation::Query).find(|p| {
            matches!(
                p.name.as_str(),
                "limit" | "per_page" | "page_size" | "page_limit"
            )
        });
        let limit_injected = limit_param.is_some();
        let query_params = match limit_param {
            Some(p) => vec![(p.name.clone(), "20".to_string())],
            None => vec![],
        };

        let args = RequestArgs {
            path_params: Default::default(),
            query_params,
            headers: vec![],
            body: None,
            no_auth: false,
        };
        let seq = ctx.spawn_request(self.bundle.project.clone(), endpoint, args);

        panel.request_seq = Some(seq);
        panel.limit_injected = limit_injected;
        panel.state = PanelState::Loading;
    }
}
