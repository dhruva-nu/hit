//! External-editor round trip and credential-prompt key handling.

use crossterm::event::{KeyCode, KeyEvent};

use super::text_input::TextInput;
use super::{AppCtx, Modal};

/// Suspend the TUI, run $EDITOR on the seed text, and return the edited text
/// (None when unchanged or anything failed — failures land in the log).
pub fn run_external_editor(terminal: &mut ratatui::DefaultTerminal, seed: &str) -> Option<String> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let path = std::env::temp_dir().join(format!("hitpoint-body-{}.json", std::process::id()));
    if let Err(e) = std::fs::write(&path, seed) {
        tracing::warn!(error = %e, "failed to write editor temp file");
        return None;
    }

    let result = tokio::task::block_in_place(|| spawn_editor(&editor, &path));
    let _ = std::fs::remove_file(&path);
    let _ = terminal.clear(); // force a full repaint after the alt-screen round trip

    match result {
        Ok(text) => {
            let text = text?;
            (text.trim() != seed.trim()).then_some(text)
        }
        Err(e) => {
            tracing::warn!(error = %e, "terminal suspend/resume failed");
            None
        }
    }
}

/// Leave the alt screen, run the editor on `path`, then restore the alt
/// screen. Returns the file contents on a clean exit, None otherwise.
fn spawn_editor(editor: &str, path: &std::path::Path) -> std::io::Result<Option<String>> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;

    // $EDITOR may carry arguments ("code -w").
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(path)
        .status();

    crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode()?;

    match status {
        Ok(s) if s.success() => Ok(Some(std::fs::read_to_string(path)?)),
        Ok(s) => {
            tracing::info!(status = ?s.code(), "editor exited non-zero; discarding");
            Ok(None)
        }
        Err(e) => {
            tracing::warn!(error = %e, editor, "failed to launch editor");
            Ok(None)
        }
    }
}

/// Keys while a modal is up: info modals dismiss on any key; prompt modals
/// behave like a one-line editor and answer the waiting auth task.
pub fn handle_modal_key(key: KeyEvent, ctx: &mut AppCtx) {
    match ctx.modal.take() {
        Some(Modal::Info { .. }) | None => {}
        Some(Modal::Prompt {
            label,
            secret,
            input,
            respond,
        }) => {
            let mut buffer = TextInput::new(input);
            match key.code {
                KeyCode::Enter => {
                    let _ = respond.send(Ok(buffer.into_string()));
                }
                KeyCode::Esc => {
                    let _ = respond.send(Err(crate::error::AuthError::Credential(
                        "login cancelled".into(),
                    )));
                }
                key_code => {
                    match key_code {
                        KeyCode::Backspace => buffer.backspace(),
                        KeyCode::Char(c) => buffer.insert_char(c),
                        _ => {}
                    }
                    ctx.modal = Some(Modal::Prompt {
                        label,
                        secret,
                        input: buffer.into_string(),
                        respond,
                    });
                }
            }
        }
    }
}
