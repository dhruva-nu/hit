//! The request form screen: navigate flattened rows, edit values inline,
//! Shift+X to null/exclude, `e` for $EDITOR, Ctrl+S to send, `p` for reference panel.

mod input;
mod nav;
mod panel;
mod preview;
mod render;
mod row;
mod submit;

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use serde_json::Value;

use super::{Action, Screen, response::ResponseView};
use crate::model::Endpoint;
use crate::tui::form::FormState;
use crate::tui::text_input::TextInput;
use crate::tui::{AppCtx, AppMsg, SpecBundle};

use panel::{PanelState, RightPanel};

pub struct RequestForm {
    bundle: Arc<SpecBundle>,
    endpoint: Endpoint,
    form: FormState,
    /// Inline editor state: (row, buffer). Some = editing.
    editor: Option<(usize, TextInput)>,
    scroll: usize,
    /// `i` toggles the endpoint docs pane (description + expected response).
    show_docs: bool,
    /// Reference panel (None when the current endpoint is GET or there are no GETs).
    right_panel: Option<RightPanel>,
    /// Whether the reference panel is currently open.
    panel_visible: bool,
}

impl RequestForm {
    pub fn new(bundle: Arc<SpecBundle>, endpoint: Endpoint) -> Self {
        let form = FormState::new(&endpoint);
        let right_panel = RightPanel::for_endpoint(&bundle, &endpoint);
        Self {
            bundle,
            endpoint,
            form,
            editor: None,
            scroll: 0,
            show_docs: false,
            right_panel,
            panel_visible: false,
        }
    }
}

impl Screen for RequestForm {
    fn title(&self) -> String {
        format!(
            "projects ▸ {} ▸ {} {}",
            self.bundle.project, self.endpoint.method, self.endpoint.path
        )
    }

    fn meta(&self) -> Option<String> {
        self.endpoint.summary.clone()
    }

    fn key_hints(&self) -> Vec<(&'static str, &'static str)> {
        if self.editor.is_some() {
            return vec![("enter", "commit"), ("esc", "cancel")];
        }

        // Picker is focused — show only picker hints.
        if self.panel_visible
            && self
                .right_panel
                .as_ref()
                .is_some_and(|p| matches!(p.state, PanelState::Picker { .. }))
        {
            return vec![("↑↓", "pick GET"), ("enter", "load"), ("p / esc", "close")];
        }

        // The footer is narrow; swap `e $EDITOR` ↔ `p preview` so the total
        // width stays the same (~112 chars) and `p` is always visible.
        let mut hints = vec![
            ("enter", "edit/toggle"),
            ("X", "null/exclude"),
            ("a/d", "array +/-"),
            ("i", "docs"),
        ];

        if self.right_panel.is_some() {
            if self.panel_visible {
                hints.push(("p", "re-pick"));
            } else {
                hints.push(("p", "preview"));
            }
        } else if self.endpoint.body.is_some() {
            hints.push(("e", "$EDITOR"));
        }

        hints.push(("ctrl+s", "send"));
        hints.push(("esc", "back"));
        hints
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppCtx) -> Action {
        if self.editor.is_some() {
            return self.handle_editor_key(key, ctx);
        }

        if let Some(action) = self.handle_panel_scroll(key) {
            return action;
        }

        // When the picker is active all navigation goes to it.
        if self.picker_active() {
            return self.handle_picker_key(key, ctx);
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            return self.submit(ctx);
        }

        self.handle_form_key(key, ctx)
    }

    fn handle_msg(&mut self, msg: &AppMsg, _ctx: &mut AppCtx) -> Action {
        if let AppMsg::Response {
            request_seq,
            result,
        } = msg
            && let Some(panel) = &mut self.right_panel
            && panel.request_seq == Some(*request_seq)
        {
            panel.state = match result {
                Ok(response) => PanelState::Done {
                    response: response.clone(),
                    scroll: 0,
                },
                Err(msg) => PanelState::Error(msg.clone()),
            };
        }
        Action::None
    }

    fn handle_editor_result(&mut self, text: Option<String>, ctx: &mut AppCtx) -> Action {
        if let Some(text) = text {
            match serde_json::from_str::<Value>(&text) {
                Ok(value) => {
                    self.form.hydrate_body(&self.endpoint, &value);
                    ctx.set_status("body updated from editor");
                }
                Err(e) => ctx.show_error(format!("editor result is not valid JSON: {e}")),
            }
        }
        Action::None
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &AppCtx) {
        self.draw_form(frame, area, ctx);
    }
}
