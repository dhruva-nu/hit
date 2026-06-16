//! Example value generation for schema nodes: fills defaults, emits
//! `<type:format>` placeholders for scalars, and records optional/nullable
//! paths while descending. Shared by `template.rs`.

use serde_json::{Map, Value, json};

use super::{Field, SchemaNode};

/// How deep example bodies auto-expand before degrading to `{}`/`[]`.
const MAX_EXAMPLE_DEPTH: usize = 6;

/// Build an example value for a schema node, recording optional/nullable
/// paths as we descend. `path` is the dotted location of this node's parent.
pub(super) fn example_value(
    node: &SchemaNode,
    path: &str,
    optional: &mut Vec<String>,
    nullable: &mut Vec<String>,
    depth: usize,
) -> Value {
    if depth > MAX_EXAMPLE_DEPTH {
        return degraded(node);
    }
    match node {
        SchemaNode::Object { fields, additional } => {
            object_example(fields, additional, path, optional, nullable, depth)
        }
        SchemaNode::Array { item, .. } => {
            let item_path = format!("{path}[]");
            json!([example_value(
                item,
                &item_path,
                optional,
                nullable,
                depth + 1
            )])
        }
        SchemaNode::OneOf { variants } => variants
            .first()
            .map(|v| example_value(&v.node, path, optional, nullable, depth + 1))
            .unwrap_or(Value::Null),
        _ => placeholder(node),
    }
}

/// Example for an object node: visit each writable field, recording its
/// optional/nullable status, and illustrate open maps with one `<key>`.
fn object_example(
    fields: &[Field],
    additional: &Option<Box<SchemaNode>>,
    path: &str,
    optional: &mut Vec<String>,
    nullable: &mut Vec<String>,
    depth: usize,
) -> Value {
    let mut map = Map::new();
    for field in fields {
        if field.read_only {
            continue;
        }
        let child_path = join_path(path, &field.name);
        if !field.required {
            optional.push(child_path.clone());
        }
        if field.nullable {
            nullable.push(child_path.clone());
        }
        let value = field_example(field, &child_path, optional, nullable, depth);
        map.insert(field.name.clone(), value);
    }
    if fields.is_empty()
        && let Some(extra) = additional
    {
        // Open map: show one illustrative key.
        map.insert(
            "<key>".to_string(),
            example_value(extra, path, optional, nullable, depth + 1),
        );
    }
    Value::Object(map)
}

fn field_example(
    field: &Field,
    child_path: &str,
    optional: &mut Vec<String>,
    nullable: &mut Vec<String>,
    depth: usize,
) -> Value {
    if let Some(default) = &field.default {
        return default.clone();
    }
    example_value(&field.schema, child_path, optional, nullable, depth + 1)
}

/// Leaf placeholder: enum/const pick a real value; scalars get a
/// `<type:format>` marker the caller is expected to replace.
pub(super) fn placeholder(node: &SchemaNode) -> Value {
    match node {
        SchemaNode::String {
            enum_values: Some(values),
            ..
        } => values
            .first()
            .map(|v| Value::String(v.clone()))
            .unwrap_or(Value::Null),
        SchemaNode::String {
            format: Some(f), ..
        } => Value::String(format!("<string:{f}>")),
        SchemaNode::String { .. } => Value::String("<string>".to_string()),
        SchemaNode::Integer {
            enum_values: Some(values),
            ..
        } => values.first().map(|v| json!(v)).unwrap_or(Value::Null),
        SchemaNode::Integer { .. } => Value::String("<integer>".to_string()),
        SchemaNode::Number { .. } => Value::String("<number>".to_string()),
        SchemaNode::Boolean => Value::Bool(false),
        SchemaNode::Const { value } => value.clone(),
        SchemaNode::Any => json!({}),
        SchemaNode::Object { .. } | SchemaNode::Array { .. } | SchemaNode::OneOf { .. } => {
            degraded(node)
        }
    }
}

fn degraded(node: &SchemaNode) -> Value {
    match node {
        SchemaNode::Array { .. } => json!([]),
        _ => json!({}),
    }
}

fn join_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}.{name}")
    }
}
