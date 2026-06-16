//! Reference-panel navigation: scrolling a loaded response and driving the
//! GET picker.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::panel::PanelState;
use super::{Action, RequestForm};
use crate::tui::AppCtx;

impl RequestForm {
    /// When the panel shows a response, ↑/↓/j/k scroll it. Alt+↑/↓ also scroll
    /// (retained for muscle-memory / when the picker is open). Returns `Some`
    /// when the key was consumed by panel scrolling.
    pub(super) fn handle_panel_scroll(&mut self, key: KeyEvent) -> Option<Action> {
        let panel_is_done = self.panel_visible
            && self
                .right_panel
                .as_ref()
                .is_some_and(|p| matches!(p.state, PanelState::Done { .. }));
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if !((panel_is_done || alt) && self.panel_visible) {
            return None;
        }
        let panel = self.right_panel.as_mut()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if panel_is_done || alt => {
                if let PanelState::Done { scroll, .. } = &mut panel.state {
                    *scroll = scroll.saturating_sub(1);
                }
                Some(Action::None)
            }
            KeyCode::Down | KeyCode::Char('j') if panel_is_done || alt => {
                if let PanelState::Done { scroll, .. } = &mut panel.state {
                    *scroll = scroll.saturating_add(1);
                }
                Some(Action::None)
            }
            _ => None,
        }
    }

    /// True when the reference picker has focus and should receive navigation.
    pub(super) fn picker_active(&self) -> bool {
        self.panel_visible
            && self
                .right_panel
                .as_ref()
                .is_some_and(|p| matches!(p.state, PanelState::Picker { .. }))
    }

    /// Handle keys while the picker is the active focus.
    pub(super) fn handle_picker_key(&mut self, key: KeyEvent, ctx: &mut AppCtx) -> Action {
        match key.code {
            KeyCode::Char('p') | KeyCode::Esc => {
                self.panel_visible = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(panel) = &mut self.right_panel
                    && let PanelState::Picker { selected } = &mut panel.state
                {
                    *selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(panel) = &mut self.right_panel
                    && let PanelState::Picker { selected } = &mut panel.state
                {
                    let max = panel.get_endpoints.len().saturating_sub(1);
                    if *selected < max {
                        *selected += 1;
                    }
                }
            }
            KeyCode::Enter => {
                self.fire_reference_get(ctx);
            }
            _ => {}
        }
        Action::None
    }
}
