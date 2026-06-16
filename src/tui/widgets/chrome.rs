//! Frame chrome: header breadcrumb, key-chip footer, the rounded content
//! panel, and the pane-separator rule.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph};

use crate::tui::theme;

/// Top bar: app badge + breadcrumb (` › `-separated, last segment bright)
/// and right-aligned meta info.
pub fn draw_header(frame: &mut Frame, area: Rect, breadcrumb: &str, meta: Option<&str>) {
    let mut spans = vec![
        Span::styled(
            " hitpoint ",
            Style::new()
                .fg(theme::BADGE_FG)
                .bg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ];
    let segments: Vec<&str> = breadcrumb.split(" ▸ ").collect();
    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        if i > 0 {
            spans.push(Span::styled(" › ", theme::dim()));
        }
        spans.push(Span::styled(
            segment.to_string(),
            if last {
                theme::bold(theme::text())
            } else {
                theme::soft()
            },
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);

    if let Some(meta) = meta {
        let meta_line = Line::from(Span::styled(format!("{meta} "), theme::dim()));
        frame.render_widget(Paragraph::new(meta_line).right_aligned(), area);
    }
}

/// Bottom bar: status message (left), key chips (right-flowing after it).
pub fn draw_footer(frame: &mut Frame, area: Rect, hints: &[(&str, &str)], status: Option<&str>) {
    let mut spans = Vec::new();
    if let Some(status) = status {
        spans.push(Span::styled("● ", Style::new().fg(theme::YELLOW)));
        spans.push(Span::styled(
            format!("{status}   "),
            Style::new().fg(theme::YELLOW),
        ));
    }
    for (key, label) in hints {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::new()
                .fg(theme::CYAN)
                .bg(theme::SEL_BG)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}   "), theme::dim()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The rounded panel every screen draws inside. Returns the inner area.
pub fn content_panel(frame: &mut Frame, area: Rect) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border())
        .padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

/// Horizontal rule used to separate panes.
pub fn rule(width: u16) -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(width as usize),
        Style::new().fg(theme::SEL_BG),
    ))
}
