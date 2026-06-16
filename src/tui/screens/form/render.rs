//! Form rendering: the scrolling row list layout, the info strip, and the
//! docs pane. Per-row rendering lives in [`super::row`].

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::RequestForm;
use crate::tui::{AppCtx, theme, widgets};

impl RequestForm {
    pub(super) fn draw_form(&mut self, frame: &mut Frame, area: Rect, ctx: &AppCtx) {
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
            area = self.draw_docs_pane(frame, area);
        }

        let [list_area, info_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(area);
        self.draw_rows(frame, list_area);
        self.draw_info_strip(frame, info_area);
    }

    /// Render the docs pane at the bottom; returns the remaining area above it.
    fn draw_docs_pane(&self, frame: &mut Frame, area: Rect) -> Rect {
        let docs = widgets::endpoint_docs_lines(&self.endpoint);
        let pane_height = (docs.len() as u16 + 2).clamp(4, area.height * 2 / 3);
        let [rest, pane] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(pane_height)]).areas(area);
        let mut pane_lines = vec![widgets::rule(pane.width)];
        pane_lines.extend(docs);
        frame.render_widget(Paragraph::new(pane_lines), pane);
        rest
    }

    /// Draw the scrolling list of visible rows, keeping the cursor in view.
    fn draw_rows(&mut self, frame: &mut Frame, list_area: Rect) {
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
    }

    /// Info strip: cursor row name · type · flags — description.
    fn draw_info_strip(&self, frame: &mut Frame, info_area: Rect) {
        let Some(row) = self.form.rows.get(self.form.cursor) else {
            return;
        };
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
