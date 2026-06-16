//! The request form screen: navigate flattened rows, edit values inline,
//! Shift+X to null/exclude, `e` for $EDITOR, Ctrl+S to send, `p` for reference panel.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use serde_json::Value;

use super::{Action, Screen, response::ResponseView};
use crate::auth::AuthManager;
use crate::config;
use crate::http::{ApiResponse, RequestArgs, RequestExecutor};
use crate::model::{Endpoint, ParamLocation};
use crate::tui::form::{FormState, RowKind, RowState};
use crate::tui::{AppCtx, AppMsg, SpecBundle, theme, widgets};

// ---------- Right reference panel ----------

enum PanelState {
    Picker { selected: usize },
    Loading,
    Done { response: ApiResponse, scroll: u16 },
    Error(String),
}

struct RightPanel {
    /// Indices into `bundle.spec.endpoints` where `method == "GET"`.
    get_endpoints: Vec<usize>,
    /// Sequence id of the in-flight or most recent reference GET.
    request_seq: Option<u64>,
    /// Whether the fired GET had a `limit` / `per_page` / `page_size` injected.
    limit_injected: bool,
    state: PanelState,
}

// ---------- RequestForm ----------

pub struct RequestForm {
    bundle: Arc<SpecBundle>,
    endpoint: Endpoint,
    form: FormState,
    /// Inline editor state: (row, text). Some = editing.
    editor: Option<(usize, String)>,
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

        let right_panel = if endpoint.method != "GET" {
            // Prefer GET endpoints that share at least one tag with this endpoint;
            // fall back to all GETs when the current endpoint carries no tags.
            let current_tags = &endpoint.tags;
            let get_endpoints: Vec<usize> = bundle
                .spec
                .endpoints
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.method == "GET"
                        && (current_tags.is_empty()
                            || e.tags.iter().any(|t| current_tags.contains(t)))
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
        } else {
            None
        };

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

    fn submit(&mut self, ctx: &mut AppCtx) -> Action {
        let serialized = match self.form.serialize() {
            Err(e) => {
                self.form.cursor = e.row;
                ctx.set_status(e.message);
                return Action::None;
            }
            Ok(s) => s,
        };

        ctx.request_seq += 1;
        let seq = ctx.request_seq;
        let services = ctx.services.clone();
        let tx = ctx.tx.clone();
        let interactor = ctx.interactor();
        let project_name = self.bundle.project.clone();
        let endpoint = self.endpoint.clone();
        let args = RequestArgs {
            path_params: serialized.path_params,
            query_params: serialized.query_params,
            headers: serialized.headers,
            body: serialized.body,
            no_auth: false,
        };

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

        Action::Push(Box::new(ResponseView::loading(
            seq,
            self.bundle.clone(),
            self.endpoint.method.clone(),
            self.endpoint.path.clone(),
        )))
    }

    /// Fire the GET endpoint currently selected in the picker.
    fn fire_reference_get(&mut self, ctx: &mut AppCtx) {
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

        ctx.request_seq += 1;
        let seq = ctx.request_seq;
        let services = ctx.services.clone();
        let tx = ctx.tx.clone();
        let interactor = ctx.interactor();
        let project_name = self.bundle.project.clone();

        let args = RequestArgs {
            path_params: Default::default(),
            query_params,
            headers: vec![],
            body: None,
            no_auth: false,
        };

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

        panel.request_seq = Some(seq);
        panel.limit_injected = limit_injected;
        panel.state = PanelState::Loading;
    }

    fn handle_editor_key(&mut self, key: KeyEvent, ctx: &mut AppCtx) -> Action {
        let (row, mut text) = self.editor.take().expect("editor checked by caller");
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => {
                if let Err(message) = self.form.commit_text(row, &text) {
                    ctx.set_status(message);
                    self.editor = Some((row, text));
                }
            }
            KeyCode::Backspace => {
                text.pop();
                self.editor = Some((row, text));
            }
            KeyCode::Char(c) => {
                text.push(c);
                self.editor = Some((row, text));
            }
            _ => self.editor = Some((row, text)),
        }
        Action::None
    }

    /// Seed JSON for external editing: current body, leniently serialized.
    fn editor_seed(&self) -> String {
        let body = self.form.body_for_editing();
        serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string())
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

        if let Some(panel) = &self.right_panel {
            if self.panel_visible {
                hints.push(("p", "re-pick"));
            } else {
                hints.push(("p", "preview"));
            }
            let _ = panel; // suppress unused warning
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

        // When the panel is open and showing a response, ↑/↓/j/k scroll it.
        // Alt+↑/↓ also scroll (retained for muscle-memory / when picker is open).
        let panel_is_done = self.panel_visible
            && self
                .right_panel
                .as_ref()
                .is_some_and(|p| matches!(p.state, PanelState::Done { .. }));
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if panel_is_done || alt {
            if let Some(panel) = &mut self.right_panel {
                if self.panel_visible {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') if panel_is_done || alt => {
                            if let PanelState::Done { scroll, .. } = &mut panel.state {
                                *scroll = scroll.saturating_sub(1);
                            }
                            return Action::None;
                        }
                        KeyCode::Down | KeyCode::Char('j') if panel_is_done || alt => {
                            if let PanelState::Done { scroll, .. } = &mut panel.state {
                                *scroll = scroll.saturating_add(1);
                            }
                            return Action::None;
                        }
                        _ => {}
                    }
                }
            }
        }

        // When the picker is active all navigation goes to it.
        let picker_active = self.panel_visible
            && self
                .right_panel
                .as_ref()
                .is_some_and(|p| matches!(p.state, PanelState::Picker { .. }));
        if picker_active {
            return self.handle_picker_key(key, ctx);
        }

        let cursor = self.form.cursor;
        let has_rows = !self.form.rows.is_empty();

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            return self.submit(ctx);
        }

        match key.code {
            // Panel toggle / cycle.
            KeyCode::Char('p') if self.right_panel.is_some() => {
                if self.panel_visible {
                    // Done / Loading / Error → return to picker.
                    if let Some(panel) = &mut self.right_panel {
                        panel.request_seq = None;
                        panel.state = PanelState::Picker { selected: 0 };
                    }
                } else {
                    self.panel_visible = true;
                }
            }

            KeyCode::Up => self.form.move_cursor(-1),
            KeyCode::Down => self.form.move_cursor(1),
            KeyCode::Char('k') => self.form.move_cursor(-1),
            KeyCode::Char('j') => self.form.move_cursor(1),
            KeyCode::Enter if has_rows => {
                let kind = self.form.rows[cursor].kind.clone();
                match kind {
                    RowKind::Scalar | RowKind::RawJson => {
                        self.editor = Some((cursor, self.form.text_of(cursor)));
                    }
                    RowKind::Bool
                    | RowKind::Enum(_)
                    | RowKind::ObjectHeader
                    | RowKind::ArrayHeader => self.form.toggle(cursor),
                    _ => {}
                }
            }
            KeyCode::Char(' ') if has_rows => self.form.toggle(cursor),
            KeyCode::Left | KeyCode::Right if has_rows => {
                if matches!(self.form.rows[cursor].kind, RowKind::Enum(_)) {
                    self.form.toggle(cursor);
                }
            }
            // Shift+X arrives as 'X' (modifier flags vary by terminal).
            KeyCode::Char('X') if has_rows => {
                if let Some(hint) = self.form.cycle_exclusion(cursor) {
                    ctx.set_status(hint);
                }
            }
            KeyCode::Char('x') if has_rows => self.form.reinclude(cursor),
            KeyCode::Char('a') if has_rows => {
                if self.form.rows[cursor].kind == RowKind::ArrayHeader {
                    self.form.array_append(cursor);
                }
            }
            KeyCode::Char('d') if has_rows => self.form.array_delete(cursor),
            KeyCode::Char('i') => self.show_docs = !self.show_docs,
            KeyCode::Tab if has_rows => self.form.toggle(cursor),
            KeyCode::Char('e') if self.endpoint.body.is_some() => {
                return Action::RunEditor {
                    seed: self.editor_seed(),
                };
            }
            KeyCode::Esc => return Action::Pop,
            _ => {}
        }
        Action::None
    }

    fn handle_msg(&mut self, msg: &AppMsg, _ctx: &mut AppCtx) -> Action {
        if let AppMsg::Response {
            request_seq,
            result,
        } = msg
        {
            if let Some(panel) = &mut self.right_panel {
                if panel.request_seq == Some(*request_seq) {
                    panel.state = match result {
                        Ok(response) => PanelState::Done {
                            response: response.clone(),
                            scroll: 0,
                        },
                        Err(msg) => PanelState::Error(msg.clone()),
                    };
                }
            }
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
        // Split horizontally when the reference panel is open.
        let (form_area, panel_area_opt) = if self.panel_visible && self.right_panel.is_some() {
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
                    .areas(area);
            (left, Some(right))
        } else {
            (area, None)
        };

        if let Some(panel_area) = panel_area_opt {
            self.draw_right_panel(frame, panel_area, ctx);
        }

        let mut area = form_area;

        // No params and no body (plain GETs): show a request preview plus
        // the docs instead of an empty screen.
        if self.form.rows.is_empty() {
            self.draw_request_preview(frame, area, ctx);
            return;
        }

        // Docs pane: endpoint description + declared/expected responses.
        if self.show_docs {
            let docs = widgets::endpoint_docs_lines(&self.endpoint);
            let pane_height = (docs.len() as u16 + 2).clamp(4, area.height * 2 / 3);
            let [rest, pane] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(pane_height)]).areas(area);
            area = rest;
            let mut pane_lines = vec![widgets::rule(pane.width)];
            pane_lines.extend(docs);
            frame.render_widget(Paragraph::new(pane_lines), pane);
        }

        let [list_area, info_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(area);

        let hidden = self.form.hidden_mask();
        let visible: Vec<usize> = (0..self.form.rows.len()).filter(|&i| !hidden[i]).collect();

        // Keep cursor in view.
        let cursor_pos = visible
            .iter()
            .position(|&i| i == self.form.cursor)
            .unwrap_or(0);
        let height = list_area.height as usize;
        if cursor_pos < self.scroll {
            self.scroll = cursor_pos;
        } else if height > 0 && cursor_pos >= self.scroll + height {
            self.scroll = cursor_pos + 1 - height;
        }

        // Label column width: longest visible label (incl. indent), clamped.
        let label_width = visible
            .iter()
            .map(|&i| {
                let row = &self.form.rows[i];
                row.depth as usize * 2 + row.label.len() + 1
            })
            .max()
            .unwrap_or(16)
            .clamp(16, 34);

        let lines: Vec<Line> = visible
            .iter()
            .skip(self.scroll)
            .take(height)
            .map(|&i| self.render_row(i, label_width, list_area.width))
            .collect();
        frame.render_widget(Paragraph::new(lines), list_area);

        // Info strip: cursor row name · type · flags — description.
        if let Some(row) = self.form.rows.get(self.form.cursor) {
            let mut spans = vec![
                Span::styled(format!(" {} ", row.label), theme::bold(theme::text())),
                Span::styled(format!("· {} ", row.kind_label), theme::dim()),
            ];
            if row.required {
                spans.push(Span::styled("· required ", Style::new().fg(theme::RED)));
            }
            if row.nullable {
                spans.push(Span::styled("· nullable ", Style::new().fg(theme::MAGENTA)));
            }
            if let Some(description) = &row.description {
                spans.push(Span::styled(format!("— {description}"), theme::soft()));
            }
            let rule = Line::from(Span::styled(
                "─".repeat(info_area.width as usize),
                Style::new().fg(theme::SEL_BG),
            ));
            frame.render_widget(Paragraph::new(vec![rule, Line::from(spans)]), info_area);
        }
    }
}

impl RequestForm {
    /// Handle keys while the picker is the active focus.
    fn handle_picker_key(&mut self, key: KeyEvent, ctx: &mut AppCtx) -> Action {
        match key.code {
            KeyCode::Char('p') | KeyCode::Esc => {
                self.panel_visible = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(panel) = &mut self.right_panel {
                    if let PanelState::Picker { selected } = &mut panel.state {
                        *selected = selected.saturating_sub(1);
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(panel) = &mut self.right_panel {
                    if let PanelState::Picker { selected } = &mut panel.state {
                        let max = panel.get_endpoints.len().saturating_sub(1);
                        if *selected < max {
                            *selected += 1;
                        }
                    }
                }
            }
            KeyCode::Enter => {
                self.fire_reference_get(ctx);
            }
            _ => {}
        }
        Action::None
    }

    fn draw_right_panel(&self, frame: &mut Frame, area: Rect, ctx: &AppCtx) {
        let panel = match &self.right_panel {
            Some(p) => p,
            None => return,
        };

        // Left border as visual separator from the form.
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(theme::border());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Header row: "reference" label.
        let [header_area, content_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " reference",
                theme::bold(theme::accent()),
            ))),
            header_area,
        );

        match &panel.state {
            PanelState::Picker { selected } => {
                let items: Vec<ListItem> = panel
                    .get_endpoints
                    .iter()
                    .map(|&idx| {
                        let e = &self.bundle.spec.endpoints[idx];
                        let mut spans = vec![
                            theme::method_badge(&e.method),
                            Span::raw(" "),
                            Span::styled(e.path.clone(), theme::soft()),
                        ];
                        if let Some(summary) = &e.summary {
                            spans.push(Span::styled(format!("  {summary}"), theme::dim()));
                        }
                        ListItem::new(Line::from(spans))
                    })
                    .collect();

                let list = List::new(items)
                    .highlight_style(theme::selected_row())
                    .highlight_symbol(Span::styled("▌", theme::accent()));
                let mut list_state = ListState::default().with_selected(Some(*selected));
                frame.render_stateful_widget(list, content_area, &mut list_state);
            }

            PanelState::Loading => {
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::raw(""),
                        widgets::loading_line("reference", ctx.frame),
                    ]),
                    content_area,
                );
            }

            PanelState::Error(msg) => {
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::raw(""),
                        Line::from(Span::styled(
                            format!(" ✗ {msg}"),
                            Style::new().fg(theme::RED),
                        )),
                    ]),
                    content_area,
                );
            }

            PanelState::Done { response, scroll } => {
                let mut lines: Vec<Line> = Vec::new();

                if panel.limit_injected {
                    lines.push(Line::from(Span::styled(
                        " showing first 20 results",
                        Style::new().fg(theme::YELLOW),
                    )));
                    lines.push(Line::raw(""));
                }

                // Truncate large arrays to the first 50 items for display.
                let truncated_total: Option<usize>;
                let display_body;
                if let Some(arr) = response.body.as_array()
                    && arr.len() > 50
                {
                    truncated_total = Some(arr.len());
                    display_body = Value::Array(arr[..50].to_vec());
                } else {
                    truncated_total = None;
                    display_body = response.body.clone();
                }

                let body_text = if response.body_is_json {
                    serde_json::to_string_pretty(&display_body).unwrap_or_default()
                } else {
                    display_body.as_str().unwrap_or("").to_string()
                };

                for raw_line in body_text.lines() {
                    if response.body_is_json {
                        lines.push(widgets::colorize_json_line(raw_line));
                    } else {
                        lines.push(Line::from(Span::styled(
                            raw_line.to_string(),
                            theme::soft(),
                        )));
                    }
                }

                if let Some(total) = truncated_total {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled(
                        format!(" … {total} total items, showing first 50"),
                        theme::dim(),
                    )));
                }

                frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), content_area);
            }
        }
    }

    /// Shown when the endpoint takes no input at all: what will be sent,
    /// whether auth is attached, and the endpoint docs.
    fn draw_request_preview(&self, frame: &mut Frame, area: Rect, ctx: &AppCtx) {
        let project = ctx.services.config.projects.get(&self.bundle.project);
        let url = project
            .map(|p| {
                format!(
                    "{}{}",
                    p.base_url.as_str().trim_end_matches('/'),
                    self.endpoint.path
                )
            })
            .unwrap_or_else(|| self.endpoint.path.clone());

        let mut lines = vec![
            Line::raw(""),
            Line::from(vec![
                theme::method_badge(&self.endpoint.method),
                Span::styled(format!(" {url}"), theme::bold(theme::text())),
            ]),
            Line::raw(""),
            Line::from(Span::styled(
                "this endpoint takes no parameters and no body",
                theme::dim(),
            )),
        ];

        let has_auth = project.is_some_and(|p| p.auth.is_some());
        let auth_line = match (self.endpoint.auth_required, has_auth) {
            (true, true) => Span::styled(
                "⚿ authenticated — your token will be attached",
                Style::new().fg(theme::GREEN),
            ),
            (true, false) => Span::styled(
                "⚿ endpoint requires auth, but this project has none configured",
                Style::new().fg(theme::YELLOW),
            ),
            (false, _) => Span::styled("no auth required", theme::dim()),
        };
        lines.push(Line::from(auth_line));
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(" ctrl+s ", Style::new().fg(theme::CYAN).bg(theme::SEL_BG)),
            Span::styled(" send the request", theme::soft()),
        ]));

        lines.push(Line::raw(""));
        lines.push(widgets::rule(area.width));
        lines.extend(widgets::endpoint_docs_lines(&self.endpoint));

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_row(&self, i: usize, label_width: usize, width: u16) -> Line<'_> {
        let row = &self.form.rows[i];
        let is_cursor = i == self.form.cursor;

        if row.kind == RowKind::SectionHeader {
            let label = format!("╴{}╶", row.label);
            let fill = (width as usize).saturating_sub(label.len() + 3);
            return Line::from(vec![
                Span::styled("──", Style::new().fg(theme::SEL_BG)),
                Span::styled(label, theme::bold(theme::accent())),
                Span::styled("─".repeat(fill), Style::new().fg(theme::SEL_BG)),
            ]);
        }

        let mut spans = vec![if is_cursor {
            Span::styled("▌ ", theme::accent())
        } else {
            Span::raw("  ")
        }];

        // Label cell, padded to the shared column width.
        let indent = "  ".repeat(row.depth as usize);
        let marker = if row.required { "*" } else { " " };
        let label_text = format!("{indent}{}{marker}", row.label);
        let padded = format!("{label_text:<label_width$}  ");
        let mut label_spans = vec![
            Span::styled(
                format!("{indent}{}", row.label),
                if is_cursor {
                    theme::bold(theme::text())
                } else {
                    theme::soft()
                },
            ),
            Span::styled(
                if row.required { "*" } else { " " },
                Style::new().fg(theme::RED),
            ),
        ];
        let pad = padded.len().saturating_sub(label_text.len());
        label_spans.push(Span::raw(" ".repeat(pad)));
        spans.extend(label_spans);

        // Inline editor takes over the value cell.
        if let Some((edit_row, text)) = &self.editor
            && *edit_row == i
        {
            spans.push(Span::styled(
                format!("{text}▏"),
                Style::new().fg(theme::YELLOW),
            ));
            return Line::from(spans);
        }

        let value_span = match (&row.state, &row.kind) {
            (RowState::Excluded, _) => Span::styled(
                "⊘ excluded",
                theme::dim().add_modifier(Modifier::CROSSED_OUT),
            ),
            (RowState::Null, _) => Span::styled(
                "∅ null",
                Style::new()
                    .fg(theme::MAGENTA)
                    .add_modifier(Modifier::ITALIC),
            ),
            (RowState::Empty, _) => {
                Span::styled("‹empty›", theme::dim().add_modifier(Modifier::ITALIC))
            }
            (RowState::Filled(_), RowKind::ObjectHeader) => Span::styled(
                if row.collapsed {
                    "{ … } collapsed"
                } else {
                    "{"
                },
                Style::new().fg(theme::CYAN),
            ),
            (RowState::Filled(_), RowKind::ArrayHeader) => Span::styled(
                format!("[ {} ]", self.array_len(i)),
                Style::new().fg(theme::CYAN),
            ),
            (RowState::Filled(v), RowKind::Enum(_)) => Span::styled(
                format!("◂ {} ▸", value_text(v)),
                Style::new().fg(theme::CYAN),
            ),
            (RowState::Filled(v), RowKind::Const) => {
                Span::styled(format!("{} ⚷ fixed", value_text(v)), theme::dim())
            }
            (RowState::Filled(v), _) => Span::styled(value_text(v), value_style(v)),
        };
        spans.push(value_span);

        spans.push(Span::styled(format!("  {}", row.kind_label), theme::dim()));
        Line::from(spans)
    }

    fn array_len(&self, header: usize) -> String {
        let depth = self.form.rows[header].depth + 1;
        let count = (header + 1..self.form.span_end(header))
            .filter(|&j| self.form.rows[j].depth == depth)
            .count();
        format!("{count} item{}", if count == 1 { "" } else { "s" })
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Color scalar values by JSON type (mirrors the response-body coloring).
fn value_style(value: &Value) -> Style {
    match value {
        Value::String(_) => Style::new().fg(theme::GREEN),
        Value::Number(_) => Style::new().fg(theme::ORANGE),
        Value::Bool(_) | Value::Null => Style::new().fg(theme::MAGENTA),
        _ => theme::text(),
    }
}
