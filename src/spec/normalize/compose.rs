//! Schema composition: `allOf` merging and `anyOf`/`oneOf` variant handling.

use serde_json::Value;

use crate::model::{Field, OneOfVariant, SchemaNode};

use super::Normalized;
use super::enums::is_null_schema;
use super::normalize_inner;

/// allOf merge. The common FastAPI 3.0-era pattern is
/// `allOf: [{$ref: Model}]` with default/description as siblings; the general
/// case merges object fields left-to-right (later parts override by name).
pub(super) fn merge_all_of(
    doc: &Value,
    parts: &[Value],
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    let normalized: Vec<Normalized> = parts
        .iter()
        .map(|p| normalize_inner(doc, p, visited, depth + 1))
        .collect();

    if normalized.is_empty() {
        return Normalized::any();
    }
    if normalized.len() == 1 {
        return normalized.into_iter().next().unwrap();
    }

    // If every part is an object, merge their fields.
    let all_objects = normalized
        .iter()
        .all(|n| matches!(n.node, SchemaNode::Object { .. }));
    if all_objects {
        return merge_object_parts(&normalized);
    }

    // Heterogeneous allOf: take the first concrete part.
    normalized
        .into_iter()
        .find(|n| n.node != SchemaNode::Any)
        .unwrap_or_else(Normalized::any)
}

/// Merge a set of object-typed parts: fields combine left-to-right (later
/// parts override by name); any part may contribute `additional`.
fn merge_object_parts(parts: &[Normalized]) -> Normalized {
    let mut merged_fields: Vec<Field> = Vec::new();
    let mut merged_additional = None;
    let mut nullable = false;
    for part in parts {
        nullable = nullable || part.nullable;
        if let SchemaNode::Object { fields, additional } = &part.node {
            for field in fields {
                if let Some(existing) = merged_fields.iter_mut().find(|f| f.name == field.name) {
                    *existing = field.clone();
                } else {
                    merged_fields.push(field.clone());
                }
            }
            if additional.is_some() {
                merged_additional = additional.clone();
            }
        }
    }
    Normalized {
        node: SchemaNode::Object {
            fields: merged_fields,
            additional: merged_additional,
        },
        nullable,
        ..Normalized::any()
    }
}

/// anyOf/oneOf handling: null variants set `nullable`; a single remaining
/// variant collapses (the FastAPI `Optional[T]` pattern); several remaining
/// variants become `OneOf`.
pub(super) fn normalize_variants(
    doc: &Value,
    variants: &[Value],
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    let mut nullable = false;
    let mut concrete = Vec::new();
    for variant in variants {
        if is_null_schema(variant) {
            nullable = true;
            continue;
        }
        concrete.push(normalize_inner(doc, variant, visited, depth + 1));
    }

    match concrete.len() {
        0 => Normalized {
            nullable: true,
            ..Normalized::any()
        },
        1 => {
            let mut single = concrete.into_iter().next().unwrap();
            single.nullable = single.nullable || nullable;
            single
        }
        _ => Normalized {
            node: SchemaNode::OneOf {
                variants: build_oneof_variants(concrete),
            },
            nullable,
            ..Normalized::any()
        },
    }
}

fn build_oneof_variants(concrete: Vec<Normalized>) -> Vec<OneOfVariant> {
    concrete
        .into_iter()
        .enumerate()
        .map(|(i, n)| OneOfVariant {
            label: n
                .title
                .clone()
                .unwrap_or_else(|| format!("{} #{}", n.node.kind_label(), i + 1)),
            node: n.node,
        })
        .collect()
}
