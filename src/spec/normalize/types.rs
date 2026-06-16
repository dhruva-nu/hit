//! Type-driven normalization: the `type` dispatch and `type` extraction for
//! schemas without composition keywords.

use serde_json::Value;

use crate::model::SchemaNode;

use super::Normalized;
use super::enums::{enum_ints, enum_strings, string_of};
use super::structural::{normalize_array, normalize_object};

/// Type-driven normalization for schemas without composition keywords.
/// `type` is a string in 3.0 and may be a `[T, "null"]` array in 3.1.
pub(super) fn normalize_by_type(
    doc: &Value,
    obj: &serde_json::Map<String, Value>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    let (type_name, type_nullable) = extract_type(obj.get("type"));
    let mut result = match type_name.as_deref() {
        Some("object") => normalize_object(doc, obj, visited, depth),
        Some("array") => normalize_array(doc, obj, visited, depth),
        Some(scalar @ ("string" | "integer" | "number" | "boolean" | "null")) => {
            normalize_scalar(scalar, obj)
        }
        Some(other) => {
            tracing::debug!(r#type = other, "unknown schema type; emitting Any");
            Normalized::any()
        }
        // No explicit type: infer from structure.
        None => infer_untyped(doc, obj, visited, depth),
    };

    collapse_single_enum(&mut result);
    result.nullable = result.nullable || type_nullable;
    result
}

/// Normalize a primitive `type`, carrying its constraints (enum/format/bounds).
fn normalize_scalar(scalar: &str, obj: &serde_json::Map<String, Value>) -> Normalized {
    let node = match scalar {
        "string" => SchemaNode::String {
            enum_values: enum_strings(obj.get("enum")),
            format: string_of(obj.get("format")),
        },
        "integer" => SchemaNode::Integer {
            minimum: obj.get("minimum").and_then(Value::as_i64),
            maximum: obj.get("maximum").and_then(Value::as_i64),
            enum_values: enum_ints(obj.get("enum")),
        },
        "number" => SchemaNode::Number {
            minimum: obj.get("minimum").and_then(Value::as_f64),
            maximum: obj.get("maximum").and_then(Value::as_f64),
        },
        "boolean" => SchemaNode::Boolean,
        // "null": the only standalone null type; carries nullability, no node.
        _ => {
            return Normalized {
                node: SchemaNode::Any,
                nullable: true,
                ..Normalized::any()
            };
        }
    };
    Normalized {
        node,
        ..Normalized::any()
    }
}

/// No explicit `type`: infer object/array/enum from structure, else `Any`.
fn infer_untyped(
    doc: &Value,
    obj: &serde_json::Map<String, Value>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    if obj.contains_key("properties") || obj.contains_key("additionalProperties") {
        normalize_object(doc, obj, visited, depth)
    } else if obj.contains_key("items") {
        normalize_array(doc, obj, visited, depth)
    } else if let Some(values) = enum_strings(obj.get("enum")) {
        Normalized {
            node: SchemaNode::String {
                enum_values: Some(values),
                format: None,
            },
            ..Normalized::any()
        }
    } else {
        Normalized::any()
    }
}

/// Single-value string enums behave like const (Pydantic `Literal["x"]`).
fn collapse_single_enum(result: &mut Normalized) {
    if let SchemaNode::String {
        enum_values: Some(values),
        ..
    } = &result.node
        && values.len() == 1
    {
        result.node = SchemaNode::Const {
            value: Value::String(values[0].clone()),
        };
    }
}

/// `type` may be a string ("string") or, in 3.1, an array (["string", "null"]).
fn extract_type(type_value: Option<&Value>) -> (Option<String>, bool) {
    match type_value {
        Some(Value::String(s)) => (Some(s.clone()), false),
        Some(Value::Array(items)) => {
            let mut nullable = false;
            let mut name = None;
            for item in items {
                match item.as_str() {
                    Some("null") => nullable = true,
                    Some(other) if name.is_none() => name = Some(other.to_string()),
                    _ => {}
                }
            }
            (name, nullable)
        }
        _ => (None, false),
    }
}
