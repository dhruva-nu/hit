//! Structural type normalization: objects (fields + additionalProperties) and
//! arrays (items + bounds).

use serde_json::Value;

use crate::model::{Field, SchemaNode};

use super::Normalized;
use super::normalize_inner;

pub(super) fn normalize_object(
    doc: &Value,
    obj: &serde_json::Map<String, Value>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    let required: Vec<&str> = obj
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut fields = Vec::new();
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        for (name, prop_schema) in props {
            let normalized = normalize_inner(doc, prop_schema, visited, depth + 1);
            fields.push(Field {
                name: name.clone(),
                required: required.contains(&name.as_str()),
                nullable: normalized.nullable,
                default: normalized.default,
                description: normalized.description,
                read_only: normalized.read_only,
                schema: normalized.node,
            });
        }
    }

    let additional = match obj.get("additionalProperties") {
        Some(Value::Bool(true)) => Some(Box::new(SchemaNode::Any)),
        Some(Value::Object(_)) => {
            let normalized = normalize_inner(doc, &obj["additionalProperties"], visited, depth + 1);
            Some(Box::new(normalized.node))
        }
        _ => None,
    };

    Normalized {
        node: SchemaNode::Object { fields, additional },
        ..Normalized::any()
    }
}

pub(super) fn normalize_array(
    doc: &Value,
    obj: &serde_json::Map<String, Value>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    let item = obj
        .get("items")
        .map(|items| normalize_inner(doc, items, visited, depth + 1).node)
        .unwrap_or(SchemaNode::Any);
    Normalized {
        node: SchemaNode::Array {
            item: Box::new(item),
            min_items: obj.get("minItems").and_then(Value::as_u64),
            max_items: obj.get("maxItems").and_then(Value::as_u64),
        },
        ..Normalized::any()
    }
}
