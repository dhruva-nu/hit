//! Interactive TUI: tokio event loop, screen stack, async task plumbing.
//!
//! Logs go to a file (set up in main); nothing here may write to
//! stdout/stderr while the alternate screen is active.

mod app;
mod dispatch;
mod editor;
pub mod form;
pub mod json_view;
mod render;
mod request;
pub mod screens;
mod text_input;
pub mod theme;
pub mod widgets;

use std::collections::HashMap;
use std::sync::Arc;

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::AppServices;
use screens::{Action, Screen};

pub use app::{AppCtx, AppMsg, Modal, SpecBundle, TuiInteractor};
use dispatch::{apply_action, handle_key, handle_msg};
use render::draw;

/// TUI entry point; returns the process exit code.
pub async fn run(services: AppServices, initial_project: Option<String>) -> i32 {
    // Restore the terminal even if we panic mid-draw — non-negotiable.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        default_hook(info);
    }));

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, services, initial_project).await;
    ratatui::restore();

    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tui error: {e}");
            1
        }
    }
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    services: AppServices,
    initial_project: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut ctx = AppCtx {
        services: Arc::new(services),
        tx,
        specs: HashMap::new(),
        modal: None,
        status: None,
        request_seq: 0,
        frame: 0,
    };

    let mut stack: Vec<Box<dyn Screen>> =
        vec![Box::new(screens::projects::ProjectList::new(&ctx.services))];

    // `hit tui <project>` jumps straight into the project.
    if let Some(name) = initial_project {
        if ctx.services.config.projects.contains_key(&name) {
            ctx.load_spec(&name);
            stack.push(Box::new(screens::tags::TagList::loading(name)));
        } else {
            ctx.show_error(format!("unknown project '{name}'"));
        }
    }

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(120));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        terminal.draw(|frame| draw(frame, &mut stack, &ctx))?;

        let action = tokio::select! {
            event = events.next() => match event {
                Some(Ok(Event::Key(key))) if key.kind != KeyEventKind::Release => {
                    handle_key(key, &mut stack, &mut ctx)
                }
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
                _ => Action::None, // resize triggers redraw; mouse ignored
            },
            msg = rx.recv() => match msg {
                Some(msg) => handle_msg(msg, &mut stack, &mut ctx),
                None => return Ok(()),
            },
            _ = ticker.tick() => {
                ctx.frame = ctx.frame.wrapping_add(1);
                Action::None
            }
        };

        if let Some(exit) = apply_action(action, terminal, &mut stack, &mut ctx) {
            return exit;
        }
    }
}
