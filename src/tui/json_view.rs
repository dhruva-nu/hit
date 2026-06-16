//! Shared rendering of a response body into colored lines, used by both the
//! response screen and the request-form reference panel. Large arrays are
//! truncated to the first 50 items for display; the per-line coloring reuses
//! `widgets::colorize_json_line`.

use ratatui::text::{Line, Span};
use serde_json::Value;

use super::theme;
use super::widgets;

/// Arrays longer than this are truncated for display.
const ARRAY_DISPLAY_LIMIT: usize = 50;

/// Turn a response body into colored lines. JSON bodies are pretty-printed and
/// syntax-colored; non-JSON bodies are shown as soft plain text. When `body` is
/// an array longer than 50 items, only the first 50 are rendered and a trailing
/// "… N total items" note is appended.
pub fn body_lines(body: &Value, body_is_json: bool) -> Vec<Line<'static>> {
    let (display_body, truncated_total) = truncate(body);

    let body_text = if body_is_json {
        serde_json::to_string_pretty(&display_body).unwrap_or_default()
    } else {
        display_body.as_str().unwrap_or("").to_string()
    };

    let mut lines: Vec<Line> = body_text
        .lines()
        .map(|raw| {
            if body_is_json {
                widgets::colorize_json_line(raw)
            } else {
                Line::from(Span::styled(raw.to_string(), theme::soft()))
            }
        })
        .collect();

    if let Some(total) = truncated_total {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            format!(" … {total} total items, showing first {ARRAY_DISPLAY_LIMIT}"),
            theme::dim(),
        )));
    }
    lines
}

/// Clamp an oversized array body to the first 50 items, reporting the original
/// length so the caller can note the truncation.
fn truncate(body: &Value) -> (Value, Option<usize>) {
    if let Some(arr) = body.as_array()
        && arr.len() > ARRAY_DISPLAY_LIMIT
    {
        (
            Value::Array(arr[..ARRAY_DISPLAY_LIMIT].to_vec()),
            Some(arr.len()),
        )
    } else {
        (body.clone(), None)
    }
}
