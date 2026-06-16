//! Small leaf helpers: enum extraction, null detection, string coercion.

use serde_json::Value;

pub(super) fn is_null_schema(schema: &Value) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("null")
}

pub(super) fn string_of(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

pub(super) fn enum_strings(value: Option<&Value>) -> Option<Vec<String>> {
    let values: Vec<String> = value?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

pub(super) fn enum_ints(value: Option<&Value>) -> Option<Vec<i64>> {
    let values: Vec<i64> = value?
        .as_array()?
        .iter()
        .filter_map(Value::as_i64)
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}
