//! Per-row rendering: the cursor marker, label cell, value cell, and the
//! section-divider line.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::RequestForm;
use crate::tui::form::{FormRow, RowKind, RowState};
use crate::tui::theme;

impl RequestForm {
    pub(super) fn render_row(&self, i: usize, label_width: usize, width: u16) -> Line<'_> {
        let row = &self.form.rows[i];
        let is_cursor = i == self.form.cursor;

        if row.kind == RowKind::SectionHeader {
            return section_header_line(&row.label, width);
        }

        let mut spans = vec![if is_cursor {
            Span::styled("▌ ", theme::accent())
        } else {
            Span::raw("  ")
        }];
        spans.extend(self.label_cell(row, is_cursor, label_width));

        // Inline editor takes over the value cell.
        if let Some((edit_row, buffer)) = &self.editor
            && *edit_row == i
        {
            spans.push(Span::styled(
                format!("{}▏", buffer.as_str()),
                Style::new().fg(theme::YELLOW),
            ));
            return Line::from(spans);
        }

        spans.push(self.value_cell(row, i));
        spans.push(Span::styled(format!("  {}", row.kind_label), theme::dim()));
        Line::from(spans)
    }

    /// The label cell, padded to the shared column width.
    fn label_cell(&self, row: &FormRow, is_cursor: bool, label_width: usize) -> Vec<Span<'static>> {
        let indent = "  ".repeat(row.depth as usize);
        let marker = if row.required { "*" } else { " " };
        let label_text = format!("{indent}{}{marker}", row.label);
        let padded = format!("{label_text:<label_width$}  ");
        let mut spans = vec![
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
        spans.push(Span::raw(" ".repeat(pad)));
        spans
    }

    /// The value cell, styled by row state and kind.
    fn value_cell(&self, row: &FormRow, i: usize) -> Span<'static> {
        match (&row.state, &row.kind) {
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
        }
    }

    fn array_len(&self, header: usize) -> String {
        let depth = self.form.rows[header].depth + 1;
        let count = (header + 1..self.form.span_end(header))
            .filter(|&j| self.form.rows[j].depth == depth)
            .count();
        format!("{count} item{}", if count == 1 { "" } else { "s" })
    }
}

/// A section divider row: `── ╴label╶ ───…`.
fn section_header_line(label: &str, width: u16) -> Line<'static> {
    let label = format!("╴{label}╶");
    let fill = (width as usize).saturating_sub(label.len() + 3);
    Line::from(vec![
        Span::styled("──", Style::new().fg(theme::SEL_BG)),
        Span::styled(label, theme::bold(theme::accent())),
        Span::styled("─".repeat(fill), Style::new().fg(theme::SEL_BG)),
    ])
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
