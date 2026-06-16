//! Construction: flatten an endpoint's params and body schema into rows,
//! and rebuild the body section from edited JSON.

use serde_json::Value;

use crate::model::{BodySpec, Endpoint, ParamLocation, SchemaNode};

use super::schema::{push_field, scalar_kind};
use super::types::{RowKind, RowState, Section};
use super::{FormRow, FormState};

impl FormState {
    pub fn new(endpoint: &Endpoint) -> Self {
        let mut rows = Vec::new();
        let mut body_is_object = true;

        for (location, section) in [
            (ParamLocation::Path, Section::Path),
            (ParamLocation::Query, Section::Query),
            (ParamLocation::Header, Section::Header),
        ] {
            let params: Vec<_> = endpoint.params_in(location).collect();
            if params.is_empty() {
                continue;
            }
            rows.push(section_header(section));
            for param in params {
                rows.push(param_row(section, param));
            }
        }

        if let Some(body) = &endpoint.body {
            rows.push(section_header(Section::Body));
            push_body_section(&mut rows, body, None, &mut body_is_object);
        }

        let mut form = Self {
            rows,
            cursor: 0,
            body_is_object,
        };
        form.clamp_cursor_to_interactive(1);
        form
    }

    /// Rebuild the body section from a JSON value (after $EDITOR editing).
    pub fn hydrate_body(&mut self, endpoint: &Endpoint, value: &Value) {
        let Some(body) = &endpoint.body else { return };
        // Drop existing body rows.
        if let Some(start) = self
            .rows
            .iter()
            .position(|r| r.section == Section::Body && r.kind == RowKind::SectionHeader)
        {
            self.rows.truncate(start);
        }
        self.rows.push(section_header(Section::Body));
        let mut body_is_object = true;
        push_body_section(&mut self.rows, body, Some(value), &mut body_is_object);
        self.body_is_object = body_is_object;
        self.clamp_cursor_to_interactive(1);
    }
}

/// Push the body rows, optionally hydrating from `value`. An object body
/// becomes one row per field; any other schema becomes a single root row.
fn push_body_section(
    rows: &mut Vec<FormRow>,
    body: &BodySpec,
    hydrate: Option<&Value>,
    body_is_object: &mut bool,
) {
    match &body.schema {
        SchemaNode::Object { fields, .. } if !fields.is_empty() => {
            for field in fields {
                if field.read_only {
                    continue;
                }
                push_field(rows, field, 0, hydrate);
            }
        }
        other => {
            *body_is_object = false;
            super::node::push_node(
                rows,
                "body".into(),
                0,
                other,
                body.required,
                false,
                None,
                None,
                hydrate.map(|v| Some(v.clone())),
            );
        }
    }
}

fn section_header(section: Section) -> FormRow {
    FormRow {
        section,
        label: section.label().to_string(),
        depth: 0,
        kind: RowKind::SectionHeader,
        state: RowState::Empty,
        required: false,
        nullable: false,
        kind_label: String::new(),
        description: None,
        schema: SchemaNode::Any,
        collapsed: false,
        saved: None,
    }
}

fn param_row(section: Section, param: &crate::model::Param) -> FormRow {
    let state = match &param.default {
        Some(default) if !default.is_null() => RowState::Filled(default.clone()),
        _ => RowState::Empty,
    };
    FormRow {
        section,
        label: param.name.clone(),
        depth: 0,
        kind: scalar_kind(&param.schema),
        state,
        required: param.required,
        nullable: param.nullable,
        kind_label: param.schema.kind_label(),
        description: param.description.clone(),
        schema: param.schema.clone(),
        collapsed: false,
        saved: None,
    }
}
