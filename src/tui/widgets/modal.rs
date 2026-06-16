//! Modal overlays: info/error boxes and the credential prompt (which masks
//! secret input), plus the centered-rect helper they share.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap};

use crate::tui::Modal;
use crate::tui::theme;

pub fn draw_modal(frame: &mut Frame, modal: &Modal) {
    match modal {
        Modal::Info { title, body } => draw_info(frame, title, body),
        Modal::Prompt {
            label,
            secret,
            input,
            ..
        } => draw_prompt(frame, label, *secret, input),
    }
}

fn draw_info(frame: &mut Frame, title: &str, body: &str) {
    let is_error = title == "error";
    let color = if is_error { theme::RED } else { theme::ACCENT };
    let width = (frame.area().width.saturating_sub(8)).min(64);
    let body_lines = (body.len() as u16 / width.max(1)).saturating_add(3);
    let area = centered_fixed(frame.area(), width, body_lines.clamp(5, 14));
    frame.render_widget(Clear, area);
    let icon = if is_error { "✗" } else { "i" };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            format!(" {icon} {title} "),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::new().fg(color))
        .padding(Padding::new(1, 1, 0, 0));
    let mut lines = vec![Line::raw("")];
    lines.push(Line::from(Span::styled(body.to_string(), theme::text())));
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(block);
    frame.render_widget(paragraph, area);
}

fn draw_prompt(frame: &mut Frame, label: &str, secret: bool, input: &str) {
    let area = centered_fixed(frame.area(), 56, 7);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            " ⚿ login ",
            Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::new().fg(theme::ACCENT))
        .padding(Padding::new(1, 1, 0, 0));
    let shown = if secret {
        "•".repeat(input.chars().count())
    } else {
        input.to_string()
    };
    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled(label.to_string(), theme::bold(theme::text()))),
        Line::from(vec![
            Span::styled("❯ ", theme::accent()),
            Span::styled(
                shown,
                Style::new()
                    .fg(theme::FG)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled("▏", theme::accent()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" enter ", chip()),
            Span::styled(" submit   ", theme::dim()),
            Span::styled(" esc ", chip()),
            Span::styled(" cancel", theme::dim()),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn chip() -> Style {
    Style::new().fg(theme::CYAN).bg(theme::SEL_BG)
}

/// A centered rect with a fixed size, clamped to the parent.
pub fn centered_fixed(parent: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width.min(parent.width))])
        .flex(Flex::Center)
        .areas(parent);
    let [area] = Layout::vertical([Constraint::Length(height.min(parent.height))])
        .flex(Flex::Center)
        .areas(area);
    area
}
