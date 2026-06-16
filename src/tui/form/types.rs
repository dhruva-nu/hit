//! Plain data types describing a form row: its location, kind, and tri-state.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Path,
    Query,
    Header,
    Body,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Path => "path params",
            Section::Query => "query params",
            Section::Header => "headers",
            Section::Body => "body",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RowKind {
    SectionHeader,
    Scalar,
    Bool,
    Enum(Vec<String>),
    Const,
    /// Any / OneOf / open maps: edited as raw JSON text.
    RawJson,
    ObjectHeader,
    ArrayHeader,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RowState {
    Filled(Value),
    Empty,
    Null,
    Excluded,
}

/// Submit-time validation failure.
#[derive(Debug, PartialEq)]
pub struct SubmitError {
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct SerializedForm {
    pub path_params: std::collections::BTreeMap<String, String>,
    pub query_params: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
}
