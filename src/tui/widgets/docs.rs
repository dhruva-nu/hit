//! Endpoint documentation block: docstring, declared responses, and an
//! example body for the first successful response.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::json::colorize_json_line;
use crate::tui::theme;

/// Docs block for an endpoint: docstring description plus declared
/// responses with an example body for the success response.
pub fn endpoint_docs_lines(endpoint: &crate::model::Endpoint) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let doc = endpoint
        .description
        .clone()
        .or_else(|| endpoint.summary.clone());
    match doc {
        Some(text) => {
            for raw in text.lines().filter(|l| !l.trim().is_empty()) {
                lines.push(Line::from(Span::styled(
                    raw.trim().to_string(),
                    theme::soft(),
                )));
            }
        }
        None => lines.push(Line::from(Span::styled("(no description)", theme::dim()))),
    }

    if endpoint.responses.is_empty() {
        return lines;
    }

    push_responses(&mut lines, endpoint);
    push_example_body(&mut lines, endpoint);
    lines
}

/// The declared response status codes with their descriptions.
fn push_responses(lines: &mut Vec<Line<'static>>, endpoint: &crate::model::Endpoint) {
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "responses",
        theme::bold(theme::accent()),
    )));
    for response in &endpoint.responses {
        let status_span = match response.status.parse::<u16>() {
            Ok(code) => theme::status_badge(code),
            Err(_) => Span::styled(
                format!(" {} ", response.status),
                Style::new().fg(theme::BADGE_FG).bg(theme::DIM),
            ),
        };
        lines.push(Line::from(vec![
            Span::raw(" "),
            status_span,
            Span::styled(
                format!("  {}", response.description.as_deref().unwrap_or("")),
                theme::soft(),
            ),
        ]));
    }
}

/// Example body of the first successful response that has a schema.
fn push_example_body(lines: &mut Vec<Line<'static>>, endpoint: &crate::model::Endpoint) {
    if let Some(success) = endpoint
        .responses
        .iter()
        .find(|r| r.is_success() && r.schema.is_some())
        && let Some(schema) = &success.schema
    {
        let example = crate::model::example_of(schema);
        if let Ok(pretty) = serde_json::to_string_pretty(&example) {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                format!("example {} body", success.status),
                theme::bold(theme::accent()),
            )));
            for raw in pretty.lines() {
                lines.push(colorize_json_line(&format!(" {raw}")));
            }
        }
    }
}
