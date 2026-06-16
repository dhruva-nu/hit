//! Endpoint list for a tag (or the whole project), with `/` filtering.

mod render;

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use super::{Action, Screen, form::RequestForm, move_selection};
use crate::tui::{AppCtx, SpecBundle};

pub struct EndpointList {
    bundle: Arc<SpecBundle>,
    tag: Option<String>,
    selected: usize,
    filter: String,
    filtering: bool,
}

impl EndpointList {
    pub fn new(bundle: Arc<SpecBundle>, tag: Option<String>) -> Self {
        Self {
            bundle,
            tag,
            selected: 0,
            filter: String::new(),
            filtering: false,
        }
    }

    /// Indices into `bundle.spec.endpoints` after tag + filter narrowing.
    fn visible(&self) -> Vec<usize> {
        let needle = self.filter.to_ascii_lowercase();
        self.bundle
            .spec
            .endpoints
            .iter()
            .enumerate()
            .filter(|(_, e)| match &self.tag {
                Some(tag) => {
                    e.tags.iter().any(|t| t == tag) || (e.tags.is_empty() && tag == "untagged")
                }
                None => true,
            })
            .filter(|(_, e)| {
                needle.is_empty()
                    || e.id.to_ascii_lowercase().contains(&needle)
                    || e.path.to_ascii_lowercase().contains(&needle)
                    || e.summary
                        .as_deref()
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .contains(&needle)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

impl Screen for EndpointList {
    fn title(&self) -> String {
        let scope = self.tag.as_deref().unwrap_or("all endpoints");
        format!("projects ▸ {} ▸ {scope}", self.bundle.project)
    }

    fn meta(&self) -> Option<String> {
        Some(format!("{} endpoints", self.visible().len()))
    }

    fn key_hints(&self) -> Vec<(&'static str, &'static str)> {
        if self.filtering {
            vec![("type", "filter"), ("enter", "apply"), ("esc", "clear")]
        } else {
            vec![
                ("↑↓", "select"),
                ("enter", "open"),
                ("/", "filter"),
                ("esc", "back"),
            ]
        }
    }

    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppCtx) -> Action {
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filtering = false;
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => self.filter.push(c),
                _ => {}
            }
            self.selected = 0;
            return Action::None;
        }

        let visible = self.visible();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_selection(&mut self.selected, visible.len(), -1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_selection(&mut self.selected, visible.len(), 1);
                Action::None
            }
            KeyCode::Char('/') => {
                self.filtering = true;
                Action::None
            }
            KeyCode::Enter => match visible.get(self.selected) {
                Some(&idx) => {
                    let endpoint = self.bundle.spec.endpoints[idx].clone();
                    Action::Push(Box::new(RequestForm::new(self.bundle.clone(), endpoint)))
                }
                None => Action::None,
            },
            KeyCode::Esc => Action::Pop,
            _ => Action::None,
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, _ctx: &AppCtx) {
        self.render(frame, area);
    }
}
