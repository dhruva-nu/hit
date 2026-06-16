//! Key handling for the request form: the inline value editor and the normal
//! form navigation. Reference-panel navigation lives in [`super::nav`].

use crossterm::event::{KeyCode, KeyEvent};

use super::panel::PanelState;
use super::{Action, RequestForm};
use crate::tui::AppCtx;
use crate::tui::form::RowKind;
use crate::tui::text_input::TextInput;

impl RequestForm {
    /// Inline value editor: accumulate chars, commit on Enter, cancel on Esc.
    pub(super) fn handle_editor_key(&mut self, key: KeyEvent, ctx: &mut AppCtx) -> Action {
        let (row, mut buffer) = self.editor.take().expect("editor checked by caller");
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => {
                if let Err(message) = self.form.commit_text(row, buffer.as_str()) {
                    ctx.set_status(message);
                    self.editor = Some((row, buffer));
                }
            }
            KeyCode::Backspace => {
                buffer.backspace();
                self.editor = Some((row, buffer));
            }
            KeyCode::Char(c) => {
                buffer.insert_char(c);
                self.editor = Some((row, buffer));
            }
            _ => self.editor = Some((row, buffer)),
        }
        Action::None
    }

    /// Normal form navigation: cursor movement, edit/toggle, exclusion, arrays.
    pub(super) fn handle_form_key(&mut self, key: KeyEvent, ctx: &mut AppCtx) -> Action {
        let cursor = self.form.cursor;
        let has_rows = !self.form.rows.is_empty();

        match key.code {
            // Panel toggle / cycle.
            KeyCode::Char('p') if self.right_panel.is_some() => {
                if self.panel_visible {
                    // Done / Loading / Error → return to picker.
                    if let Some(panel) = &mut self.right_panel {
                        panel.request_seq = None;
                        panel.state = PanelState::Picker { selected: 0 };
                    }
                } else {
                    self.panel_visible = true;
                }
            }

            KeyCode::Up => self.form.move_cursor(-1),
            KeyCode::Down => self.form.move_cursor(1),
            KeyCode::Char('k') => self.form.move_cursor(-1),
            KeyCode::Char('j') => self.form.move_cursor(1),
            KeyCode::Enter if has_rows => self.activate_row(cursor),
            KeyCode::Char(' ') if has_rows => self.form.toggle(cursor),
            KeyCode::Left | KeyCode::Right if has_rows => {
                if matches!(self.form.rows[cursor].kind, RowKind::Enum(_)) {
                    self.form.toggle(cursor);
                }
            }
            // Shift+X arrives as 'X' (modifier flags vary by terminal).
            KeyCode::Char('X') if has_rows => {
                if let Some(hint) = self.form.cycle_exclusion(cursor) {
                    ctx.set_status(hint);
                }
            }
            KeyCode::Char('x') if has_rows => self.form.reinclude(cursor),
            KeyCode::Char('a') if has_rows => {
                if self.form.rows[cursor].kind == RowKind::ArrayHeader {
                    self.form.array_append(cursor);
                }
            }
            KeyCode::Char('d') if has_rows => self.form.array_delete(cursor),
            KeyCode::Char('i') => self.show_docs = !self.show_docs,
            KeyCode::Tab if has_rows => self.form.toggle(cursor),
            KeyCode::Char('e') if self.endpoint.body.is_some() => {
                return Action::RunEditor {
                    seed: self.editor_seed(),
                };
            }
            KeyCode::Esc => return Action::Pop,
            _ => {}
        }
        Action::None
    }

    /// Enter on a row: open the inline editor for scalars, toggle the rest.
    fn activate_row(&mut self, cursor: usize) {
        let kind = self.form.rows[cursor].kind.clone();
        match kind {
            RowKind::Scalar | RowKind::RawJson => {
                self.editor = Some((cursor, TextInput::new(self.form.text_of(cursor))));
            }
            RowKind::Bool | RowKind::Enum(_) | RowKind::ObjectHeader | RowKind::ArrayHeader => {
                self.form.toggle(cursor)
            }
            _ => {}
        }
    }
}
