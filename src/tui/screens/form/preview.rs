//! Drawing for the reference panel (picker / loading / error / response) and
//! the no-input request preview.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use super::RequestForm;
use super::panel::{PanelState, RightPanel};
use crate::http::ApiResponse;
use crate::tui::{AppCtx, json_view, theme, widgets};

impl RequestForm {
    pub(super) fn draw_right_panel(&self, frame: &mut Frame, area: Rect, ctx: &AppCtx) {
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
            PanelState::Picker { selected } => self.draw_picker(frame, content_area, *selected),
            PanelState::Loading => frame.render_widget(
                Paragraph::new(vec![
                    Line::raw(""),
                    widgets::loading_line("reference", ctx.frame),
                ]),
                content_area,
            ),
            PanelState::Error(msg) => frame.render_widget(
                Paragraph::new(vec![
                    Line::raw(""),
                    Line::from(Span::styled(
                        format!(" ✗ {msg}"),
                        Style::new().fg(theme::RED),
                    )),
                ]),
                content_area,
            ),
            PanelState::Done { response, scroll } => {
                let lines = self.reference_body_lines(panel, response);
                frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), content_area);
            }
        }
    }

    /// The list of candidate GET endpoints in the picker.
    fn draw_picker(&self, frame: &mut Frame, area: Rect, selected: usize) {
        let panel = self.right_panel.as_ref().expect("picker has a panel");
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
        let mut list_state = ListState::default().with_selected(Some(selected));
        frame.render_stateful_widget(list, area, &mut list_state);
    }

    /// Colored body lines for a loaded reference response, with the optional
    /// "showing first 20 results" note when a limit was injected.
    fn reference_body_lines(
        &self,
        panel: &RightPanel,
        response: &ApiResponse,
    ) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();
        if panel.limit_injected {
            lines.push(Line::from(Span::styled(
                " showing first 20 results",
                Style::new().fg(theme::YELLOW),
            )));
            lines.push(Line::raw(""));
        }
        lines.extend(json_view::body_lines(&response.body, response.body_is_json));
        lines
    }

    /// Shown when the endpoint takes no input at all: what will be sent,
    /// whether auth is attached, and the endpoint docs.
    pub(super) fn draw_request_preview(&self, frame: &mut Frame, area: Rect, ctx: &AppCtx) {
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
}
