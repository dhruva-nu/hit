//! Top-level frame layout: header, screen body, footer, and modal overlay.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use super::screens::Screen;
use super::{AppCtx, widgets};

pub fn draw(frame: &mut Frame, stack: &mut [Box<dyn Screen>], ctx: &AppCtx) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let top = stack.last_mut().expect("stack is never empty");
    widgets::draw_header(frame, header, &top.title(), top.meta().as_deref());
    let inner = widgets::content_panel(frame, body);
    top.draw(frame, inner, ctx);
    widgets::draw_footer(frame, footer, &top.key_hints(), ctx.status.as_deref());

    if let Some(modal) = &ctx.modal {
        widgets::draw_modal(frame, modal);
    }
}
