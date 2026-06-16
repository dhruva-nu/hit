//! JSON syntax coloring for pretty-printed bodies and the spinner loading line.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::theme;
use crate::tui::theme::spinner;

pub fn loading_line(label: &str, frame: u64) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{} ", spinner(frame)),
            Style::new().fg(theme::ACCENT),
        ),
        Span::styled(
            format!("loading {label}…"),
            Style::new()
                .fg(theme::FG_SOFT)
                .add_modifier(Modifier::ITALIC),
        ),
    ])
}

/// Per-line JSON syntax coloring for pretty-printed bodies: keys cyan,
/// strings green, numbers orange, booleans/null magenta, punctuation dim.
pub fn colorize_json_line(line: &str) -> Line<'static> {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    let mut spans = vec![Span::raw(indent.to_string())];

    let mut value_part = rest;
    // `"key": value` — split off the key when present.
    if rest.starts_with('"')
        && let Some(colon) = rest.find("\": ")
    {
        spans.push(Span::styled(
            rest[..colon + 1].to_string(),
            Style::new().fg(theme::CYAN),
        ));
        spans.push(Span::styled(": ".to_string(), theme::dim()));
        value_part = &rest[colon + 3..];
    }

    let trailing_comma = value_part.ends_with(',');
    let value = value_part.trim_end_matches(',');
    let style = match value.chars().next() {
        Some('"') => Style::new().fg(theme::GREEN),
        Some(c) if c.is_ascii_digit() || c == '-' => Style::new().fg(theme::ORANGE),
        Some('t') | Some('f') | Some('n') => Style::new().fg(theme::MAGENTA),
        Some('{') | Some('}') | Some('[') | Some(']') => theme::dim(),
        _ => theme::text(),
    };
    spans.push(Span::styled(value.to_string(), style));
    if trailing_comma {
        spans.push(Span::styled(",".to_string(), theme::dim()));
    }
    Line::from(spans)
}
