//! Rendering of a completed response: status badge, framework error detail,
//! optional headers, and the pretty JSON body.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::ResponseView;
use crate::http::ApiResponse;
use crate::spec::adapter::adapter_for;
use crate::tui::{AppCtx, json_view, theme};

impl ResponseView {
    pub(super) fn render_response(
        &self,
        response: &ApiResponse,
        ctx: &AppCtx,
    ) -> Vec<Line<'static>> {
        let mut lines = vec![self.badge_line(response)];
        self.push_error_lines(&mut lines, response, ctx);
        if self.show_headers {
            push_header_lines(&mut lines, response);
        }
        lines.push(Line::raw(""));
        lines.extend(json_view::body_lines(&response.body, response.body_is_json));
        lines
    }

    /// Status badge, method, URL, and latency.
    fn badge_line(&self, response: &ApiResponse) -> Line<'static> {
        Line::from(vec![
            theme::status_badge(response.status),
            Span::raw(" "),
            theme::method_badge(&response.method),
            Span::styled(format!(" {}", response.url), theme::bold(theme::text())),
            Span::styled(format!("  ⏱ {} ms", response.latency_ms), theme::dim()),
        ])
    }

    /// Framework-aware error rendering (FastAPI 422 detail).
    fn push_error_lines(
        &self,
        lines: &mut Vec<Line<'static>>,
        response: &ApiResponse,
        ctx: &AppCtx,
    ) {
        if response.status >= 400
            && let Some(project) = ctx.services.config.projects.get(&self.bundle.project)
            && let Some(error_lines) =
                adapter_for(project.framework).render_error_lines(response.status, &response.body)
        {
            lines.push(Line::raw(""));
            for error_line in error_lines {
                lines.push(Line::from(vec![
                    Span::styled("  ✗ ", Style::new().fg(theme::RED)),
                    Span::styled(error_line, Style::new().fg(theme::YELLOW)),
                ]));
            }
        }
    }
}

fn push_header_lines(lines: &mut Vec<Line<'static>>, response: &ApiResponse) {
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  ── headers ──", theme::dim())));
    for (name, value) in &response.headers {
        lines.push(Line::from(vec![
            Span::styled(format!("  {name}"), Style::new().fg(theme::CYAN)),
            Span::styled(": ", theme::dim()),
            Span::styled(value.clone(), theme::soft()),
        ]));
    }
}
