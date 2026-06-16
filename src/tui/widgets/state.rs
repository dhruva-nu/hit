//! Centered empty-state panel for screens with nothing to show.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::modal::centered_fixed;
use crate::tui::theme;

/// Centered empty-state panel.
pub fn empty_state(frame: &mut Frame, area: Rect, headline: &str, hint: &str) {
    let region = centered_fixed(area, (hint.len() as u16 + 6).max(40), 5);
    let lines = vec![
        Line::from(Span::styled(
            headline.to_string(),
            theme::bold(theme::soft()),
        ))
        .centered(),
        Line::raw(""),
        Line::from(Span::styled(hint.to_string(), Style::new().fg(theme::CYAN))).centered(),
    ];
    frame.render_widget(Paragraph::new(lines), region);
}
