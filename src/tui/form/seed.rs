//! Producing body JSON to seed the external editor: like `serialize`, but
//! required `Empty` fields become type-appropriate blanks instead of blocking.

use serde_json::{Map, Value};

use crate::model::SchemaNode;

use super::types::{RowKind, RowState, Section};
use super::{FormRow, FormState};

impl FormState {
    /// Body JSON for external editing.
    pub fn body_for_editing(&self) -> Value {
        let Some(first) = self.first_body_row() else {
            return Value::Object(Map::new());
        };
        let form = FormState {
            rows: self.clone_rows_lenient(),
            cursor: 0,
            body_is_object: self.body_is_object,
        };
        form.serialize_body_root(first)
            .ok()
            .and_then(|(v, _)| v)
            .unwrap_or(Value::Object(Map::new()))
    }

    fn first_body_row(&self) -> Option<usize> {
        let header = self
            .rows
            .iter()
            .position(|r| r.section == Section::Body && r.kind == RowKind::SectionHeader)?;
        (header + 1 < self.rows.len()).then_some(header + 1)
    }

    /// Copy of the rows with required-Empty leaves filled with type-appropriate
    /// blanks so lenient serialization can't fail.
    fn clone_rows_lenient(&self) -> Vec<FormRow> {
        self.rows
            .iter()
            .map(|row| {
                let mut row = row.clone();
                if row.state == RowState::Empty {
                    row.state = RowState::Filled(blank_value(&row.schema));
                }
                row
            })
            .collect()
    }
}

/// A type-appropriate blank for lenient (editor-seed) serialization.
fn blank_value(schema: &SchemaNode) -> Value {
    match schema {
        SchemaNode::String { .. } => Value::String(String::new()),
        SchemaNode::Integer { .. } => Value::from(0),
        SchemaNode::Number { .. } => Value::from(0.0),
        SchemaNode::Boolean => Value::Bool(false),
        SchemaNode::Array { .. } | SchemaNode::Object { .. } => Value::Bool(true), // header marker
        _ => Value::Object(Map::new()),
    }
}
