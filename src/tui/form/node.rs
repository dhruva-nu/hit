//! The recursive body-tree builder: turn a schema node into a header row
//! plus rows for its children, choosing each row's initial tri-state.

use serde_json::Value;

use crate::model::SchemaNode;

use super::FormRow;
use super::schema::{push_array_item, push_field, scalar_kind};
use super::types::{RowKind, RowState, Section};

/// `hydrated`: None = no hydration (use defaults); Some(None) = hydration
/// says the key is absent; Some(Some(v)) = hydration provides a value.
#[allow(clippy::too_many_arguments)]
pub(super) fn push_node(
    rows: &mut Vec<FormRow>,
    label: String,
    depth: u16,
    schema: &SchemaNode,
    required: bool,
    nullable: bool,
    description: Option<String>,
    default: Option<Value>,
    hydrated: Option<Option<Value>>,
) {
    let kind = scalar_kind(schema);
    let state = initial_state(&kind, required, &default, &hydrated);
    let header_idx = rows.len();
    rows.push(FormRow {
        section: Section::Body,
        label,
        depth,
        kind: kind.clone(),
        state,
        required,
        nullable,
        kind_label: schema.kind_label(),
        description,
        schema: schema.clone(),
        collapsed: false,
        saved: None,
    });

    let child_value = match &hydrated {
        Some(Some(v)) => Some(v.clone()),
        _ => None,
    };
    push_children(
        rows,
        header_idx,
        &kind,
        schema,
        &default,
        child_value,
        depth,
    );
}

/// Emit the descendant rows of a container (or fill in a const value).
fn push_children(
    rows: &mut Vec<FormRow>,
    header_idx: usize,
    kind: &RowKind,
    schema: &SchemaNode,
    default: &Option<Value>,
    child_value: Option<Value>,
    depth: u16,
) {
    match (kind, schema) {
        (RowKind::ObjectHeader, SchemaNode::Object { fields, .. }) => {
            for field in fields {
                if field.read_only {
                    continue;
                }
                push_field(rows, field, depth + 1, child_value.as_ref());
            }
        }
        (RowKind::ArrayHeader, SchemaNode::Array { item, .. }) => {
            let items: Vec<Value> = child_value
                .as_ref()
                .and_then(|v| v.as_array().cloned())
                .or_else(|| default.as_ref().and_then(|d| d.as_array().cloned()))
                .unwrap_or_default();
            for (idx, item_value) in items.iter().enumerate() {
                push_array_item(rows, item, idx, depth + 1, Some(item_value.clone()));
            }
        }
        (RowKind::Const, SchemaNode::Const { value }) => {
            rows[header_idx].state = RowState::Filled(value.clone());
        }
        _ => {}
    }
}

fn initial_state(
    kind: &RowKind,
    required: bool,
    default: &Option<Value>,
    hydrated: &Option<Option<Value>>,
) -> RowState {
    match hydrated {
        Some(None) => {
            return if required {
                RowState::Empty
            } else {
                RowState::Excluded
            };
        }
        Some(Some(Value::Null)) => return RowState::Null,
        Some(Some(value)) => {
            return match kind {
                RowKind::ObjectHeader | RowKind::ArrayHeader => RowState::Filled(Value::Bool(true)),
                _ => RowState::Filled(value.clone()),
            };
        }
        None => {}
    }
    state_from_default(kind, required, default)
}

/// Initial state with no hydration source, driven by the schema default.
fn state_from_default(kind: &RowKind, required: bool, default: &Option<Value>) -> RowState {
    match (kind, default) {
        // Non-null default on a container: materialize it (children rows are
        // generated from the default by the caller).
        (RowKind::ObjectHeader | RowKind::ArrayHeader, Some(d)) if !d.is_null() => {
            RowState::Filled(Value::Bool(true))
        }
        (RowKind::ObjectHeader | RowKind::ArrayHeader, _) => {
            if required {
                RowState::Filled(Value::Bool(true))
            } else {
                RowState::Excluded
            }
        }
        // `Optional[X] = None`: omit by default so server defaults (and PATCH
        // exclude_unset semantics) apply; Shift+X reaches explicit null.
        (_, Some(Value::Null)) => {
            if required {
                RowState::Null
            } else {
                RowState::Excluded
            }
        }
        (_, Some(default)) => RowState::Filled(default.clone()),
        (_, None) => RowState::Empty,
    }
}
