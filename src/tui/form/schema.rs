//! Mapping a schema node to a row kind, and the leaf/field builders that
//! defer the recursive work to [`super::node::push_node`].

use serde_json::Value;

use crate::model::{Field, SchemaNode};

use super::FormRow;
use super::node::push_node;
use super::types::RowKind;

pub(super) fn scalar_kind(schema: &SchemaNode) -> RowKind {
    match schema {
        SchemaNode::Boolean => RowKind::Bool,
        SchemaNode::String {
            enum_values: Some(values),
            ..
        } => RowKind::Enum(values.clone()),
        SchemaNode::Integer {
            enum_values: Some(values),
            ..
        } => RowKind::Enum(values.iter().map(|v| v.to_string()).collect()),
        SchemaNode::Const { .. } => RowKind::Const,
        SchemaNode::String { .. } | SchemaNode::Integer { .. } | SchemaNode::Number { .. } => {
            RowKind::Scalar
        }
        SchemaNode::Object { fields, .. } if !fields.is_empty() => RowKind::ObjectHeader,
        SchemaNode::Array { .. } => RowKind::ArrayHeader,
        // Open maps, Any, OneOf: raw JSON editing.
        _ => RowKind::RawJson,
    }
}

/// Push rows for one object field (and its children), optionally hydrating
/// from `parent_value` (the JSON object containing this field).
pub(super) fn push_field(
    rows: &mut Vec<FormRow>,
    field: &Field,
    depth: u16,
    parent_value: Option<&Value>,
) {
    let value = parent_value.map(|v| v.get(&field.name));
    let hydrated = match value {
        // Hydration source present: field missing -> excluded/empty.
        Some(None) => Some(None),
        Some(Some(v)) => Some(Some(v.clone())),
        None => None,
    };
    push_node(
        rows,
        field.name.clone(),
        depth,
        &field.schema,
        field.required,
        field.nullable,
        field.description.clone(),
        field.default.clone(),
        hydrated,
    );
}

pub(super) fn push_array_item(
    rows: &mut Vec<FormRow>,
    item_schema: &SchemaNode,
    index: usize,
    depth: u16,
    value: Option<Value>,
) {
    push_node(
        rows,
        format!("[{index}]"),
        depth,
        item_schema,
        true, // array items are "required" within the array
        false,
        None,
        None,
        value.map(Some),
    );
}
