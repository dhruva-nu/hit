//! Body attachment by content type and path-template filling.

use std::collections::BTreeMap;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;

use crate::error::RequestError;
use crate::model::Endpoint;

/// Encode everything except RFC 3986 unreserved characters.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Attach the body using the endpoint's declared content type.
pub(crate) fn attach_body(
    request: reqwest::RequestBuilder,
    endpoint: &Endpoint,
    body: &Value,
) -> Result<reqwest::RequestBuilder, RequestError> {
    let content_type = endpoint
        .body
        .as_ref()
        .map(|b| b.content_type.as_str())
        .unwrap_or("application/json");

    let parsed = unwrap_stringified(body);
    let body = parsed.as_ref().unwrap_or(body);

    if content_type == "application/x-www-form-urlencoded" {
        return attach_form(request, body);
    }
    if content_type.starts_with("multipart/") {
        return Err(RequestError::InvalidBody(
            "multipart/form-data bodies are not supported yet".into(),
        ));
    }
    Ok(request.json(body))
}

/// Some MCP clients deliver the JSON body as a stringified blob rather than a
/// JSON object. Unwrap one layer of string encoding so we don't send a
/// double-encoded body (which servers reject as "not a valid object").
fn unwrap_stringified(body: &Value) -> Option<Value> {
    match body {
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(v) if !v.is_string() => Some(v),
            _ => None,
        },
        _ => None,
    }
}

fn attach_form(
    request: reqwest::RequestBuilder,
    body: &Value,
) -> Result<reqwest::RequestBuilder, RequestError> {
    let map = body.as_object().ok_or_else(|| {
        RequestError::InvalidBody("form-encoded endpoints need a JSON object body".into())
    })?;
    let form: Vec<(String, String)> = map
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), scalar_to_string(v)))
        .collect();
    Ok(request.form(&form))
}

fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Substitute `{param}` placeholders, percent-encoding values.
pub(crate) fn fill_path(
    template: &str,
    params: &BTreeMap<String, String>,
) -> Result<String, RequestError> {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after
            .find('}')
            .ok_or_else(|| RequestError::InvalidBody(format!("malformed path '{template}'")))?;
        let name = &after[..end];
        let value = params
            .get(name)
            .ok_or_else(|| RequestError::MissingPathParam(name.to_string()))?;
        result.extend(utf8_percent_encode(value, PATH_SEGMENT));
        rest = &after[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_and_encodes_path_params() {
        let mut params = BTreeMap::new();
        params.insert("user_id".to_string(), "a b/c".to_string());
        assert_eq!(
            fill_path("/users/{user_id}/posts", &params).unwrap(),
            "/users/a%20b%2Fc/posts"
        );
    }

    #[test]
    fn missing_path_param_errors() {
        let err = fill_path("/users/{user_id}", &BTreeMap::new()).unwrap_err();
        assert!(matches!(err, RequestError::MissingPathParam(name) if name == "user_id"));
    }
}
