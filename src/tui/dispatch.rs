//! Routing keys and async messages to the top screen, and resolving the
//! `Action`s they return (including the `$EDITOR` round-trip).

use crossterm::event::{KeyCode, KeyEvent};

use super::editor::{handle_modal_key, run_external_editor};
use super::screens::{Action, Screen};
use super::{AppCtx, AppMsg, Modal};

/// Resolve an `Action` (chasing `RunEditor` follow-ups). Returns `Some` to
/// exit the event loop, `None` to keep running.
pub(super) fn apply_action(
    action: Action,
    terminal: &mut ratatui::DefaultTerminal,
    stack: &mut Vec<Box<dyn Screen>>,
    ctx: &mut AppCtx,
) -> Option<Result<(), Box<dyn std::error::Error>>> {
    let mut action = action;
    loop {
        match action {
            Action::None => return None,
            Action::Push(screen) => {
                stack.push(screen);
                return None;
            }
            Action::Pop => {
                stack.pop();
                if stack.is_empty() {
                    return Some(Ok(()));
                }
                return None;
            }
            Action::Quit => return Some(Ok(())),
            Action::RunEditor { seed } => {
                let text = run_external_editor(terminal, &seed);
                let top = stack.last_mut().expect("stack is never empty");
                action = top.handle_editor_result(text, ctx);
            }
        }
    }
}

pub(super) fn handle_key(key: KeyEvent, stack: &mut [Box<dyn Screen>], ctx: &mut AppCtx) -> Action {
    ctx.status = None;
    if ctx.modal.is_some() {
        handle_modal_key(key, ctx);
        return Action::None;
    }
    let top = stack.last_mut().expect("stack is never empty");
    let action = top.handle_key(key, ctx);
    if matches!(action, Action::None)
        && let KeyCode::Char('q') = key.code
        && stack.len() == 1
    {
        // 'q' quits from the root screen unless the screen consumed it.
        return Action::Quit;
    }
    action
}

pub(super) fn handle_msg(msg: AppMsg, stack: &mut [Box<dyn Screen>], ctx: &mut AppCtx) -> Action {
    match msg {
        AppMsg::Prompt {
            label,
            secret,
            respond,
        } => {
            ctx.modal = Some(Modal::Prompt {
                label,
                secret,
                input: String::new(),
                respond,
            });
            Action::None
        }
        AppMsg::Notify(message) => {
            ctx.set_status(message);
            Action::None
        }
        AppMsg::Error(message) => {
            ctx.show_error(message);
            Action::None
        }
        msg => {
            if let AppMsg::SpecLoaded { project, result } = &msg
                && let Ok(bundle) = result
            {
                ctx.specs.insert(project.clone(), bundle.clone());
            }
            let top = stack.last_mut().expect("stack is never empty");
            top.handle_msg(&msg, ctx)
        }
    }
}
