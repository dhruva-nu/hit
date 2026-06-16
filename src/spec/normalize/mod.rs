//! The normalization pass: raw OpenAPI 3.0/3.1 JSON-schema fragments become
//! `SchemaNode`s. All version differences are erased here — `nullable: true`
//! (3.0), `type: ["T", "null"]` (3.1), and `anyOf: [T, {type: "null"}]`
//! (FastAPI's `Optional[T]` encoding) all collapse to the same shape.
//!
//! Malformed or unsupported schemas degrade to `SchemaNode::Any` (raw JSON
//! editing in the TUI) rather than failing the endpoint or the project.

mod compose;
mod enums;
mod overlay;
mod structural;
mod types;

use serde_json::Value;

use crate::model::SchemaNode;
use crate::spec::resolve;

use compose::{merge_all_of, normalize_variants};
use overlay::Overlay;
use types::normalize_by_type;

/// Hard recursion cap (beyond ref-cycle detection) for pathological nesting.
const MAX_DEPTH: usize = 48;

/// A normalized schema plus the field-level attributes that live alongside
/// the type in JSON Schema but belong to `Field` in our model.
#[derive(Debug, Clone)]
pub struct Normalized {
    pub node: SchemaNode,
    pub nullable: bool,
    pub default: Option<Value>,
    pub description: Option<String>,
    pub title: Option<String>,
    pub read_only: bool,
}

impl Normalized {
    pub(crate) fn any() -> Self {
        Self {
            node: SchemaNode::Any,
            nullable: false,
            default: None,
            description: None,
            title: None,
            read_only: false,
        }
    }
}

/// Entry point: normalize `schema` against the root spec document.
pub fn normalize(doc: &Value, schema: &Value) -> Normalized {
    let mut visited = Vec::new();
    normalize_inner(doc, schema, &mut visited, 0)
}

/// Thin dispatcher: peel off composition keywords (`$ref`/`allOf`/`anyOf`/
/// `const`), otherwise fall through to type-based normalization. The shared
/// sibling attributes are overlaid onto whatever the inner arm produces.
fn normalize_inner(
    doc: &Value,
    schema: &Value,
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    if depth > MAX_DEPTH {
        return Normalized::any();
    }
    // Boolean schemas (3.1): `true` = anything, `false` = nothing sensible.
    let Some(obj) = schema.as_object() else {
        return Normalized::any();
    };

    let overlay = Overlay::extract(obj);

    // --- $ref ---------------------------------------------------------
    if let Some(reference) = obj.get("$ref").and_then(Value::as_str) {
        return overlay.apply(normalize_ref(doc, schema, reference, visited, depth));
    }

    // --- allOf: merge -------------------------------------------------
    if let Some(parts) = obj.get("allOf").and_then(Value::as_array) {
        return overlay.apply(merge_all_of(doc, parts, visited, depth));
    }

    // --- anyOf / oneOf: strip null variants, collapse or branch --------
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = obj.get(key).and_then(Value::as_array) {
            return overlay.apply(normalize_variants(doc, variants, visited, depth));
        }
    }

    // --- const ---------------------------------------------------------
    if let Some(value) = obj.get("const") {
        return overlay.apply(Normalized {
            node: SchemaNode::Const {
                value: value.clone(),
            },
            ..Normalized::any()
        });
    }

    overlay.apply(normalize_by_type(doc, obj, visited, depth))
}

/// Resolve and normalize a `$ref`, cutting recursive cycles to `Any`.
fn normalize_ref(
    doc: &Value,
    schema: &Value,
    reference: &str,
    visited: &mut Vec<String>,
    depth: usize,
) -> Normalized {
    if visited.iter().any(|r| r == reference) {
        // Recursive model: cut the cycle, degrade this arm to Any.
        tracing::debug!(reference, "recursive $ref; emitting Any");
        return Normalized::any();
    }
    let (target, _) = resolve::deref(doc, schema);
    visited.push(reference.to_string());
    let inner = normalize_inner(doc, target, visited, depth + 1);
    visited.pop();
    inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn norm(schema: Value) -> Normalized {
        normalize(&json!({}), &schema)
    }

    #[test]
    fn openapi30_nullable_flag() {
        let n = norm(json!({"type": "string", "nullable": true}));
        assert!(n.nullable);
        assert!(matches!(n.node, SchemaNode::String { .. }));
    }

    #[test]
    fn openapi31_type_array_null() {
        let n = norm(json!({"type": ["string", "null"]}));
        assert!(n.nullable);
        assert!(matches!(n.node, SchemaNode::String { .. }));
    }

    #[test]
    fn fastapi_optional_anyof_null() {
        // Optional[str] on FastAPI/Pydantic v2: anyOf [string, null]
        let n = norm(json!({"anyOf": [{"type": "string"}, {"type": "null"}], "title": "Name"}));
        assert!(n.nullable);
        assert!(matches!(n.node, SchemaNode::String { .. }));
    }

    #[test]
    fn optional_union_keeps_oneof_and_nullable() {
        let n = norm(json!({
            "anyOf": [
                {"type": "string", "title": "Str"},
                {"type": "integer", "title": "Int"},
                {"type": "null"}
            ]
        }));
        assert!(n.nullable);
        let SchemaNode::OneOf { variants } = &n.node else {
            panic!("expected OneOf, got {:?}", n.node)
        };
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].label, "Str");
    }

    #[test]
    fn allof_single_ref_with_sibling_default() {
        // FastAPI 3.0-era: field with default referencing an enum model.
        let doc = json!({"components": {"schemas": {
            "Color": {"type": "string", "enum": ["red", "blue"]}
        }}});
        let schema = json!({"allOf": [{"$ref": "#/components/schemas/Color"}], "default": "red"});
        let n = normalize(&doc, &schema);
        assert_eq!(n.default, Some(json!("red")));
        assert!(matches!(&n.node, SchemaNode::String { enum_values: Some(v), .. } if v.len() == 2));
    }

    #[test]
    fn ref_resolution_and_required_orthogonal_to_nullable() {
        let doc = json!({"components": {"schemas": {
            "User": {
                "type": "object",
                "required": ["name", "nickname"],
                "properties": {
                    "name": {"type": "string"},
                    "nickname": {"anyOf": [{"type": "string"}, {"type": "null"}]},
                    "bio": {"anyOf": [{"type": "string"}, {"type": "null"}], "default": null},
                    "level": {"type": "integer", "default": 3}
                }
            }
        }}});
        let n = normalize(&doc, &json!({"$ref": "#/components/schemas/User"}));
        let SchemaNode::Object { fields, .. } = &n.node else {
            panic!("expected object")
        };
        let get = |name: &str| fields.iter().find(|f| f.name == name).unwrap();
        // str -> required, not nullable
        assert!(get("name").required && !get("name").nullable);
        // Optional[str] (no default) -> required AND nullable
        assert!(get("nickname").required && get("nickname").nullable);
        // Optional[str] = None -> optional + nullable
        assert!(!get("bio").required && get("bio").nullable);
        // int = 3 -> optional, not nullable, default kept
        let level = get("level");
        assert!(!level.required && !level.nullable);
        assert_eq!(level.default, Some(json!(3)));
    }

    #[test]
    fn recursive_ref_degrades_to_any() {
        let doc = json!({"components": {"schemas": {
            "Node": {
                "type": "object",
                "properties": {
                    "value": {"type": "string"},
                    "child": {"anyOf": [{"$ref": "#/components/schemas/Node"}, {"type": "null"}]}
                }
            }
        }}});
        let n = normalize(&doc, &json!({"$ref": "#/components/schemas/Node"}));
        let SchemaNode::Object { fields, .. } = &n.node else {
            panic!("expected object")
        };
        let child = fields.iter().find(|f| f.name == "child").unwrap();
        assert_eq!(child.schema, SchemaNode::Any);
        assert!(child.nullable);
    }

    #[test]
    fn literal_single_enum_becomes_const() {
        let n = norm(json!({"type": "string", "enum": ["fixed"]}));
        assert_eq!(
            n.node,
            SchemaNode::Const {
                value: json!("fixed")
            }
        );
    }

    #[test]
    fn malformed_schema_degrades_to_any() {
        assert_eq!(norm(json!(true)).node, SchemaNode::Any);
        assert_eq!(norm(json!({"type": 42})).node, SchemaNode::Any);
        assert_eq!(norm(json!({})).node, SchemaNode::Any);
    }

    #[test]
    fn dict_str_model_open_map() {
        let n = norm(json!({
            "type": "object",
            "additionalProperties": {"type": "integer"}
        }));
        let SchemaNode::Object { fields, additional } = &n.node else {
            panic!("expected object")
        };
        assert!(fields.is_empty());
        assert!(matches!(
            additional.as_deref(),
            Some(SchemaNode::Integer { .. })
        ));
    }
}
