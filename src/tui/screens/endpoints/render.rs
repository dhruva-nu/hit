//! Rendering for the endpoint list: search bar, hovered-endpoint docs pane,
//! and the endpoint list itself.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::EndpointList;
use crate::tui::{theme, widgets};

impl EndpointList {
    pub(super) fn render(&self, frame: &mut Frame, area: Rect) {
        let mut list_area = area;

        // Search bar (visible while typing or when a filter is applied).
        if self.filtering || !self.filter.is_empty() {
            let [search, rest] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
            list_area = rest;
            self.draw_search_bar(frame, search);
        }

        let visible = self.visible();
        if visible.is_empty() {
            widgets::empty_state(frame, list_area, "no endpoints match", "press / to refine");
            return;
        }

        // Docs pane for the hovered endpoint (description + responses).
        if let Some(&idx) = visible.get(self.selected)
            && list_area.height > 14
        {
            list_area = self.draw_docs_pane(frame, list_area, idx);
        }

        self.draw_list(frame, list_area, &visible);
    }

    fn draw_search_bar(&self, frame: &mut Frame, area: Rect) {
        let mut spans = vec![
            Span::styled(" / ", Style::new().fg(theme::CYAN).bg(theme::SEL_BG)),
            Span::raw(" "),
            Span::styled(self.filter.clone(), Style::new().fg(theme::YELLOW)),
        ];
        if self.filtering {
            spans.push(Span::styled("▏", Style::new().fg(theme::YELLOW)));
        } else {
            spans.push(Span::styled("  (esc on / clears)", theme::dim()));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Render the hovered endpoint's docs at the bottom; returns the list area.
    fn draw_docs_pane(&self, frame: &mut Frame, area: Rect, idx: usize) -> Rect {
        let endpoint = &self.bundle.spec.endpoints[idx];
        let docs = widgets::endpoint_docs_lines(endpoint);
        let pane_height = (docs.len() as u16 + 2).clamp(4, area.height / 2);
        let [rest, pane] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(pane_height)]).areas(area);
        let mut pane_lines = vec![widgets::rule(pane.width)];
        pane_lines.extend(docs);
        frame.render_widget(Paragraph::new(pane_lines), pane);
        rest
    }

    fn draw_list(&self, frame: &mut Frame, area: Rect, visible: &[usize]) {
        let items: Vec<ListItem> = visible
            .iter()
            .map(|&idx| {
                let e = &self.bundle.spec.endpoints[idx];
                let mut spans = vec![
                    theme::method_badge(&e.method),
                    Span::raw(" "),
                    Span::styled(format!("{:<36}", e.path), theme::bold(theme::text())),
                    Span::styled(
                        e.summary.clone().unwrap_or_else(|| e.id.clone()),
                        theme::dim(),
                    ),
                ];
                if e.auth_required {
                    spans.push(Span::styled("  ⚿", Style::new().fg(theme::YELLOW)));
                }
                if e.deprecated {
                    spans.push(Span::styled(
                        "  deprecated",
                        Style::new()
                            .fg(theme::RED)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(theme::selected_row())
            .highlight_symbol(Span::styled("▌", theme::accent()));
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, area, &mut state);
    }
}
