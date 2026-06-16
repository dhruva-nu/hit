//! Per-row state transitions: the Shift+X exclusion cycle, bool/enum toggles,
//! and committing text typed into the inline editor.

use serde_json::Value;

use crate::model::SchemaNode;

use super::types::{RowKind, RowState, Section};
use super::{FormRow, FormState};

impl FormState {
    /// Shift+X. Returns a status hint when the cycle is a no-op.
    pub fn cycle_exclusion(&mut self, i: usize) -> Option<&'static str> {
        let row = &mut self.rows[i];
        if !row.interactive() {
            return None;
        }
        // Params have no null on the wire: nullable acts like optional there.
        let nullable = row.nullable && row.section == Section::Body;
        let optional = !row.required;

        let next = match (&row.state, optional, nullable) {
            (RowState::Filled(_) | RowState::Empty, true, true) => RowState::Null,
            (RowState::Null, true, true) => RowState::Excluded,
            (RowState::Excluded, true, true) => restored(row),
            (RowState::Filled(_) | RowState::Empty, true, false) => RowState::Excluded,
            (RowState::Excluded, true, false) => restored(row),
            (RowState::Filled(_) | RowState::Empty, false, true) => RowState::Null,
            (RowState::Null, false, true) => restored(row),
            (_, false, false) => return Some("field is required and not nullable"),
            (state, _, _) => {
                tracing::debug!(?state, "unexpected exclusion-cycle state");
                return None;
            }
        };
        if let RowState::Filled(value) = &row.state {
            row.saved = Some(value.clone());
        }
        row.state = next;
        None
    }

    /// Plain `x`: re-include an Excluded/Null row (terminal-quirk fallback).
    pub fn reinclude(&mut self, i: usize) {
        if matches!(self.rows[i].state, RowState::Excluded | RowState::Null) {
            self.rows[i].state = restored(&self.rows[i]);
        }
    }

    /// Toggle a bool row or cycle an enum row forward.
    pub fn toggle(&mut self, i: usize) {
        let row = &mut self.rows[i];
        match &row.kind {
            RowKind::Bool => {
                let current = matches!(&row.state, RowState::Filled(Value::Bool(true)));
                row.state = RowState::Filled(Value::Bool(!current));
            }
            RowKind::Enum(values) => {
                let current = match &row.state {
                    RowState::Filled(Value::String(s)) => values
                        .iter()
                        .position(|v| v == s)
                        .map(|p| p + 1)
                        .unwrap_or(0),
                    _ => 0,
                };
                let next = values[current % values.len()].clone();
                row.state = RowState::Filled(Value::String(next));
            }
            RowKind::ObjectHeader | RowKind::ArrayHeader => match row.state {
                RowState::Excluded | RowState::Null | RowState::Empty => {
                    row.state = RowState::Filled(Value::Bool(true));
                }
                _ => row.collapsed = !row.collapsed,
            },
            _ => {}
        }
    }

    /// Commit text typed into the inline editor for row `i`.
    pub fn commit_text(&mut self, i: usize, text: &str) -> Result<(), String> {
        let row = &mut self.rows[i];
        if text.is_empty() {
            row.state = RowState::Empty;
            return Ok(());
        }
        let value = match (&row.kind, &row.schema) {
            (RowKind::RawJson, _) => {
                serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?
            }
            (_, SchemaNode::Integer { .. }) => {
                let n: i64 = text
                    .trim()
                    .parse()
                    .map_err(|_| "not an integer".to_string())?;
                Value::from(n)
            }
            (_, SchemaNode::Number { .. }) => {
                let n: f64 = text
                    .trim()
                    .parse()
                    .map_err(|_| "not a number".to_string())?;
                Value::from(n)
            }
            (_, SchemaNode::Boolean) => {
                let b: bool = text
                    .trim()
                    .parse()
                    .map_err(|_| "true or false".to_string())?;
                Value::Bool(b)
            }
            _ => Value::String(text.to_string()),
        };
        row.state = RowState::Filled(value);
        Ok(())
    }

    /// Current row text for seeding the inline editor.
    pub fn text_of(&self, i: usize) -> String {
        match &self.rows[i].state {
            RowState::Filled(Value::String(s)) => s.clone(),
            RowState::Filled(other) => {
                if self.rows[i].kind == RowKind::RawJson {
                    serde_json::to_string_pretty(other).unwrap_or_default()
                } else {
                    other.to_string()
                }
            }
            _ => String::new(),
        }
    }
}

/// The state to restore when re-including a row: its saved value, a header
/// marker for containers, or `Empty` for a scalar with nothing saved.
fn restored(row: &FormRow) -> RowState {
    match (&row.saved, &row.kind) {
        (Some(value), _) => RowState::Filled(value.clone()),
        (None, RowKind::ObjectHeader | RowKind::ArrayHeader) => RowState::Filled(Value::Bool(true)),
        (None, _) => RowState::Empty,
    }
}
