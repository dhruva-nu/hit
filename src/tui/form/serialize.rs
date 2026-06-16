//! Walk the rows back into JSON, validating required fields along the way.

use serde_json::{Map, Value};

use super::FormState;
use super::types::{RowKind, RowState, Section, SerializedForm, SubmitError};

impl FormState {
    /// Validate and serialize all sections.
    pub fn serialize(&self) -> Result<SerializedForm, SubmitError> {
        let mut out = SerializedForm::default();
        let mut i = 0;
        while i < self.rows.len() {
            let row = &self.rows[i];
            match (row.section, &row.kind) {
                (_, RowKind::SectionHeader) => i += 1,
                (Section::Body, _) => {
                    // Body rows: handled as one tree walk from here.
                    let (body, _next) = self.serialize_body_root(i)?;
                    out.body = body;
                    break;
                }
                (section, _) => {
                    if let Some(value) = self.param_value(i)? {
                        let name = row.label.clone();
                        match section {
                            Section::Path => {
                                out.path_params.insert(name, value);
                            }
                            Section::Query => out.query_params.push((name, value)),
                            Section::Header => out.headers.push((name, value)),
                            Section::Body => unreachable!(),
                        }
                    }
                    i += 1;
                }
            }
        }
        Ok(out)
    }

    fn param_value(&self, i: usize) -> Result<Option<String>, SubmitError> {
        let row = &self.rows[i];
        match &row.state {
            RowState::Filled(Value::String(s)) => Ok(Some(s.clone())),
            RowState::Filled(other) => Ok(Some(other.to_string())),
            RowState::Empty if row.required => Err(SubmitError {
                row: i,
                message: format!("required {} '{}' is empty", row.section.label(), row.label),
            }),
            _ => Ok(None),
        }
    }

    /// Serialize the whole body section starting at its first field row.
    pub(super) fn serialize_body_root(
        &self,
        first: usize,
    ) -> Result<(Option<Value>, usize), SubmitError> {
        if !self.body_is_object {
            return self.serialize_node(first);
        }
        let mut map = Map::new();
        let mut i = first;
        while i < self.rows.len() {
            let row = &self.rows[i];
            if row.kind == RowKind::SectionHeader {
                break;
            }
            if row.depth == 0 {
                let label = row.label.clone();
                let (value, next) = self.serialize_node(i)?;
                if let Some(value) = value {
                    map.insert(label, value);
                }
                i = next;
            } else {
                i += 1;
            }
        }
        Ok((Some(Value::Object(map)), i))
    }

    /// Serialize one row (and its span). Returns (None=omit, value).
    fn serialize_node(&self, i: usize) -> Result<(Option<Value>, usize), SubmitError> {
        let row = &self.rows[i];
        let end = self.span_end(i);
        match &row.state {
            RowState::Excluded => Ok((None, end)),
            RowState::Null => Ok((Some(Value::Null), end)),
            RowState::Empty => {
                if row.required {
                    Err(SubmitError {
                        row: i,
                        message: format!("required field '{}' is empty", row.label),
                    })
                } else {
                    Ok((None, end))
                }
            }
            RowState::Filled(value) => match &row.kind {
                RowKind::ObjectHeader => Ok((Some(self.serialize_object(i)?), end)),
                RowKind::ArrayHeader => Ok((Some(self.serialize_array(i)?), end)),
                _ => Ok((Some(value.clone()), end)),
            },
        }
    }

    fn serialize_object(&self, i: usize) -> Result<Value, SubmitError> {
        let mut map = Map::new();
        for child in self.direct_children(i) {
            let label = self.rows[child].label.clone();
            if let (Some(child_value), _) = self.serialize_node(child)? {
                map.insert(label, child_value);
            }
        }
        Ok(Value::Object(map))
    }

    fn serialize_array(&self, i: usize) -> Result<Value, SubmitError> {
        let mut items = Vec::new();
        for child in self.direct_children(i) {
            if let (Some(child_value), _) = self.serialize_node(child)? {
                items.push(child_value);
            }
        }
        Ok(Value::Array(items))
    }
}
