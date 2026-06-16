//! Field-level attributes that may sit beside `$ref`/`allOf`/`anyOf` and must
//! survive merging, applied as an overlay onto the normalized inner schema.

use serde_json::Value;

use super::Normalized;
use super::enums::string_of;

pub(super) struct Overlay {
    default: Option<Value>,
    description: Option<String>,
    title: Option<String>,
    read_only: bool,
    nullable: bool,
}

impl Overlay {
    pub(super) fn extract(obj: &serde_json::Map<String, Value>) -> Self {
        Self {
            default: obj.get("default").cloned(),
            description: string_of(obj.get("description")),
            title: string_of(obj.get("title")),
            read_only: obj
                .get("readOnly")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            nullable: obj
                .get("nullable")
                .and_then(Value::as_bool)
                .unwrap_or(false), // 3.0
        }
    }

    pub(super) fn apply(&self, mut inner: Normalized) -> Normalized {
        inner.default = self.default.clone().or(inner.default);
        inner.description = self.description.clone().or(inner.description);
        inner.title = self.title.clone().or(inner.title);
        inner.read_only = inner.read_only || self.read_only;
        inner.nullable = inner.nullable || self.nullable;
        inner
    }
}
