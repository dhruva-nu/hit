//! Parsers for CLI key/value, header, and body arguments.

use std::io::Read;

use serde_json::Value;

use crate::error::{HitError, RequestError};

pub(crate) fn parse_kv(input: &str) -> Result<(String, String), HitError> {
    input
        .split_once('=')
        .map(|(k, v)| (k.trim().to_string(), v.to_string()))
        .ok_or_else(|| {
            HitError::Request(RequestError::InvalidBody(format!(
                "expected name=value, got '{input}'"
            )))
        })
}

pub(crate) fn parse_header(input: &str) -> Result<(String, String), HitError> {
    input
        .split_once(':')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .ok_or_else(|| HitError::Request(RequestError::InvalidHeader(input.to_string())))
}

/// Parse --body: inline JSON, @file, or '-' for stdin.
pub(crate) fn parse_body(input: &str) -> Result<Value, HitError> {
    let raw = if input == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| HitError::Request(RequestError::InvalidBody(format!("stdin: {e}"))))?;
        buf
    } else if let Some(path) = input.strip_prefix('@') {
        std::fs::read_to_string(path)
            .map_err(|e| HitError::Request(RequestError::InvalidBody(format!("{path}: {e}"))))?
    } else {
        input.to_string()
    };
    serde_json::from_str(&raw)
        .map_err(|e| HitError::Request(RequestError::InvalidBody(e.to_string())))
}
