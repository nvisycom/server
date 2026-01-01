//! Header parsing and serialization utilities.

use std::collections::HashMap;

use serde_json::Value;

/// Converts `serde_json::Value` to `HashMap<String, String>`.
///
/// Returns an empty map if deserialization fails.
#[inline]
pub fn parse_headers(value: Value) -> HashMap<String, String> {
    serde_json::from_value(value).unwrap_or_default()
}

/// Converts `HashMap<String, String>` to `Option<serde_json::Value>`.
///
/// Returns `None` if the map is empty.
#[inline]
pub fn serialize_headers(headers: HashMap<String, String>) -> Option<Value> {
    if headers.is_empty() {
        None
    } else {
        Some(serde_json::to_value(&headers).unwrap_or_default())
    }
}

/// Converts `Option<HashMap<String, String>>` to `Option<serde_json::Value>`.
///
/// Returns `None` if the input is `None` or the map is empty.
#[inline]
pub fn serialize_headers_opt(headers: Option<HashMap<String, String>>) -> Option<Value> {
    headers.and_then(serialize_headers)
}
