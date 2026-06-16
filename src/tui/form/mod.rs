//! The request form model: the endpoint's params and body schema flattened
//! into a list of editable rows. The rows ARE the model — serialization
//! walks them back into JSON, and the Shift+X tri-state lives on each row.
//!
//! State semantics (orthogonal `required`/`nullable`, see `model::Field`):
//! - `Filled(v)`  -> sent as `v`
//! - `Empty`      -> blocks submit when required, omitted otherwise
//! - `Null`       -> sent as JSON null (only reachable when nullable)
//! - `Excluded`   -> key omitted entirely (only reachable when optional)
//!
//! The implementation is split by concern: [`types`] holds the plain data;
//! [`build`], [`schema`], and [`node`] construct rows from a schema; [`cursor`]
//! handles navigation and array ops; [`edit`] handles state transitions;
//! [`serialize`] walks rows back to JSON; and [`seed`] produces lenient JSON
//! for the external editor.

mod build;
mod cursor;
mod edit;
mod node;
mod schema;
mod seed;
mod serialize;
mod types;

pub use types::{RowKind, RowState, Section, SerializedForm, SubmitError};

use serde_json::Value;

use crate::model::SchemaNode;

#[derive(Debug, Clone)]
pub struct FormRow {
    pub section: Section,
    /// Last path segment: field name, or `[i]` for array items. Display only.
    pub label: String,
    /// Indentation inside the body tree (0 for top-level fields and params).
    pub depth: u16,
    pub kind: RowKind,
    pub state: RowState,
    pub required: bool,
    pub nullable: bool,
    pub kind_label: String,
    pub description: Option<String>,
    pub schema: SchemaNode,
    pub collapsed: bool,
    /// Value stashed when cycling away from Filled, restored on re-include.
    saved: Option<Value>,
}

impl FormRow {
    pub fn interactive(&self) -> bool {
        !matches!(self.kind, RowKind::SectionHeader | RowKind::Const)
    }
}

pub struct FormState {
    pub rows: Vec<FormRow>,
    pub cursor: usize,
    /// False when the body is a single non-object root (array/raw JSON).
    body_is_object: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Endpoint;
    use crate::spec::build;
    use serde_json::{Value, json};

    fn fixture_endpoint(id: &str) -> Endpoint {
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/fastapi_31.json"
        ))
        .unwrap();
        let doc: Value = serde_json::from_str(&raw).unwrap();
        build(&doc).unwrap().find_endpoint(id).unwrap().clone()
    }

    fn row_index(form: &FormState, label: &str) -> usize {
        form.rows.iter().position(|r| r.label == label).unwrap()
    }

    #[test]
    fn create_user_form_layout_and_initial_states() {
        let form = FormState::new(&fixture_endpoint("create_user_users__post"));
        // required str -> Empty
        assert_eq!(form.rows[row_index(&form, "email")].state, RowState::Empty);
        // enum with default -> Filled(default)
        assert_eq!(
            form.rows[row_index(&form, "role")].state,
            RowState::Filled(json!("member"))
        );
        // Optional[int] = None -> Excluded (omit; server default applies)
        assert_eq!(form.rows[row_index(&form, "age")].state, RowState::Excluded);
        // array with default [] -> materialized empty
        assert_eq!(
            form.rows[row_index(&form, "tags")].state,
            RowState::Filled(json!(true))
        );
        // optional nested object -> Excluded header with hidden children
        let address = row_index(&form, "address");
        assert_eq!(form.rows[address].kind, RowKind::ObjectHeader);
        assert_eq!(form.rows[address].state, RowState::Excluded);
        let hidden = form.hidden_mask();
        assert!(hidden[row_index(&form, "line1")]);
    }

    #[test]
    fn shift_x_cycles_per_required_nullable_matrix() {
        let mut form = FormState::new(&fixture_endpoint("create_user_users__post"));

        // optional + nullable (age, starts Excluded): full cycle
        let age = row_index(&form, "age");
        form.cycle_exclusion(age); // Excluded -> restored (nothing saved -> Empty)
        assert_eq!(form.rows[age].state, RowState::Empty);
        form.commit_text(age, "30").unwrap();
        form.cycle_exclusion(age); // Filled -> Null, saves 30
        assert_eq!(form.rows[age].state, RowState::Null);
        form.cycle_exclusion(age); // Null -> Excluded
        assert_eq!(form.rows[age].state, RowState::Excluded);
        form.cycle_exclusion(age); // Excluded -> restored Filled(30)
        assert_eq!(form.rows[age].state, RowState::Filled(json!(30)));

        // required + nullable (nickname): Filled/Empty <-> Null only
        let nickname = row_index(&form, "nickname");
        form.cycle_exclusion(nickname);
        assert_eq!(form.rows[nickname].state, RowState::Null);
        form.cycle_exclusion(nickname);
        assert_eq!(form.rows[nickname].state, RowState::Empty);

        // required, not nullable (email): no-op with hint
        let email = row_index(&form, "email");
        let hint = form.cycle_exclusion(email);
        assert!(hint.is_some());
        assert_eq!(form.rows[email].state, RowState::Empty);
    }

    #[test]
    fn submit_blocks_on_required_empty_then_serializes() {
        let mut form = FormState::new(&fixture_endpoint("create_user_users__post"));
        let err = form.serialize().unwrap_err();
        assert_eq!(err.row, row_index(&form, "email"));

        form.commit_text(row_index(&form, "email"), "neo@matrix.io")
            .unwrap();
        form.commit_text(row_index(&form, "name"), "Neo").unwrap();
        let nickname = row_index(&form, "nickname");
        form.cycle_exclusion(nickname); // required+nullable -> send null

        let serialized = form.serialize().unwrap();
        let body = serialized.body.unwrap();
        assert_eq!(
            body,
            json!({
                "email": "neo@matrix.io",
                "name": "Neo",
                "nickname": null,
                "role": "member",
                "tags": []
            })
        );
        // address and age Excluded -> keys absent
        assert!(body.get("address").is_none());
        assert!(body.get("age").is_none());
    }

    #[test]
    fn nested_object_include_and_serialize() {
        let mut form = FormState::new(&fixture_endpoint("create_user_users__post"));
        form.commit_text(row_index(&form, "email"), "a@b.c")
            .unwrap();
        form.commit_text(row_index(&form, "name"), "A").unwrap();
        form.cycle_exclusion(row_index(&form, "nickname"));

        let address = row_index(&form, "address");
        form.toggle(address); // re-include
        assert_eq!(form.rows[address].state, RowState::Filled(json!(true)));
        form.commit_text(row_index(&form, "line1"), "1 Main St")
            .unwrap();
        form.commit_text(row_index(&form, "city"), "Zion").unwrap();

        let body = form.serialize().unwrap().body.unwrap();
        assert_eq!(
            body["address"],
            json!({"line1": "1 Main St", "city": "Zion"})
        );
        // line2 optional+nullable, Empty -> omitted
        assert!(body["address"].get("line2").is_none());
    }

    #[test]
    fn array_append_fill_delete() {
        let mut form = FormState::new(&fixture_endpoint("create_item_items__post"));
        form.commit_text(row_index(&form, "sku"), "SKU-1").unwrap();
        form.commit_text(row_index(&form, "price"), "9.5").unwrap();

        let variants = row_index(&form, "variants");
        assert_eq!(form.rows[variants].kind, RowKind::ArrayHeader);
        form.array_append(variants);
        form.array_append(variants);

        // Fill both items: rows for item 0 and 1
        let names: Vec<usize> = form
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.label == "name" && r.section == Section::Body)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(names.len(), 2);
        let stocks: Vec<usize> = form
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.label == "stock")
            .map(|(i, _)| i)
            .collect();
        form.commit_text(names[0], "Red").unwrap();
        form.commit_text(stocks[0], "5").unwrap();
        form.commit_text(names[1], "Blue").unwrap();
        form.commit_text(stocks[1], "7").unwrap();

        let body = form.serialize().unwrap().body.unwrap();
        assert_eq!(
            body["variants"],
            json!([{"name": "Red", "stock": 5}, {"name": "Blue", "stock": 7}])
        );
        assert_eq!(body["kind"], json!("physical")); // const auto-filled

        // Delete item 0; item 1 renumbers and survives
        form.array_delete(names[0]);
        let body = form.serialize().unwrap().body.unwrap();
        assert_eq!(body["variants"], json!([{"name": "Blue", "stock": 7}]));
        let item_roots: Vec<&FormRow> = form
            .rows
            .iter()
            .filter(|r| r.label.starts_with('['))
            .collect();
        assert_eq!(item_roots.len(), 1);
        assert_eq!(item_roots[0].label, "[0]");
    }

    #[test]
    fn params_serialize_and_required_path_param_blocks() {
        let mut form = FormState::new(&fixture_endpoint("update_user_users__user_id__patch"));
        let err = form.serialize().unwrap_err();
        assert!(err.message.contains("user_id"));

        let user_id = row_index(&form, "user_id");
        form.commit_text(user_id, "u-42").unwrap();
        let serialized = form.serialize().unwrap();
        assert_eq!(serialized.path_params["user_id"], "u-42");
        // PATCH body: both fields Optional[...] = None -> omitted entirely,
        // which is what exclude_unset-style PATCH endpoints expect.
        assert_eq!(serialized.body.unwrap(), json!({}));
    }

    #[test]
    fn query_param_x_excludes_instead_of_null() {
        let mut form = FormState::new(&fixture_endpoint("list_users_users__get"));
        let limit = row_index(&form, "limit");
        assert_eq!(form.rows[limit].state, RowState::Filled(json!(20)));
        form.cycle_exclusion(limit); // optional, param -> Excluded (never Null)
        assert_eq!(form.rows[limit].state, RowState::Excluded);
        let serialized = form.serialize().unwrap();
        assert!(serialized.query_params.is_empty());
    }

    #[test]
    fn hydrate_body_round_trips_editor_json() {
        let endpoint = fixture_endpoint("create_user_users__post");
        let mut form = FormState::new(&endpoint);
        let edited = json!({
            "email": "trinity@matrix.io",
            "name": "Trinity",
            "nickname": null,
            "address": {"line1": "2 Side St", "city": "Zion"},
            "tags": ["ops", "pilot"]
        });
        form.hydrate_body(&endpoint, &edited);

        // role/age were absent in the edited JSON -> excluded
        assert_eq!(
            form.rows[row_index(&form, "role")].state,
            RowState::Excluded
        );
        let body = form.serialize().unwrap().body.unwrap();
        assert_eq!(body, edited);
    }

    #[test]
    fn enum_toggle_cycles_values() {
        let mut form = FormState::new(&fixture_endpoint("create_user_users__post"));
        let role = row_index(&form, "role");
        form.toggle(role); // member -> viewer
        assert_eq!(form.rows[role].state, RowState::Filled(json!("viewer")));
        form.toggle(role); // viewer -> admin (wraps)
        assert_eq!(form.rows[role].state, RowState::Filled(json!("admin")));
    }

    #[test]
    fn commit_text_validates_types() {
        let mut form = FormState::new(&fixture_endpoint("list_users_users__get"));
        let limit = row_index(&form, "limit");
        assert!(form.commit_text(limit, "abc").is_err());
        assert!(form.commit_text(limit, "50").is_ok());
        assert_eq!(form.rows[limit].state, RowState::Filled(json!(50)));
    }
}
