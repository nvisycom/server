//! Custom headers attached to a webhook delivery.
//!
//! [`WebhookHeaders`] is a validated map of header name → value: construction
//! rejects malformed names/values, so a stored value is always a set of
//! well-formed HTTP headers. Reserved, server-set names (signature, request id,
//! content type, …) are filtered at *delivery* time, not here — this type
//! guarantees well-formedness, the delivery path owns which names a webhook may
//! not override.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Json;

/// A validated set of custom webhook headers (name → value).
///
/// Backed by a `BTreeMap` so serialization is deterministic (stable key order).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct WebhookHeaders(BTreeMap<String, String>);

/// A header name or value that is not a well-formed HTTP header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidHeader {
    /// The offending header name.
    pub name: String,
    /// Why it was rejected.
    pub reason: &'static str,
}

impl std::fmt::Display for InvalidHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid header `{}`: {}", self.name, self.reason)
    }
}

impl std::error::Error for InvalidHeader {}

impl WebhookHeaders {
    /// Validates a map into typed headers, rejecting malformed names/values.
    ///
    /// A name must be a non-empty HTTP token (RFC 7230 `field-name`); a value
    /// must contain only visible ASCII plus spaces/tabs (no control characters,
    /// so a value cannot inject a second header line).
    pub fn try_new(
        headers: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, InvalidHeader> {
        let mut map = BTreeMap::new();
        for (name, value) in headers {
            if !is_valid_header_name(&name) {
                return Err(InvalidHeader {
                    name,
                    reason: "not a valid HTTP header name",
                });
            }
            if !is_valid_header_value(&value) {
                return Err(InvalidHeader {
                    name,
                    reason: "value contains control characters",
                });
            }
            map.insert(name, value);
        }
        Ok(Self(map))
    }

    /// Whether there are no headers.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Wraps these headers as a stored column, or `None` when empty.
    ///
    /// An empty set stores nothing (the column defaults to an empty object), so a
    /// webhook without custom headers carries no stored value.
    pub fn into_column(self) -> Option<Json<Self>> {
        if self.is_empty() {
            None
        } else {
            Some(Json::encode(&self))
        }
    }

    /// Iterates the headers as `(name, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Borrows the underlying ordered name → value map.
    pub fn as_map(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    /// Consumes into the underlying ordered name → value map.
    pub fn into_map(self) -> BTreeMap<String, String> {
        self.0
    }
}

/// Whether `name` is a valid HTTP header name (RFC 7230 token, non-empty).
fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

/// Whether `value` is a valid HTTP header value (visible ASCII + space/tab, no
/// control characters that could inject a new header line).
fn is_valid_header_value(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b == b'\t' || (b' '..=b'~').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::WebhookHeaders;

    fn make(name: &str, value: &str) -> Result<WebhookHeaders, super::InvalidHeader> {
        WebhookHeaders::try_new([(name.to_owned(), value.to_owned())])
    }

    #[test]
    fn accepts_a_well_formed_header() {
        let headers = make("X-Custom-Token", "abc 123\ttab").expect("valid");
        assert_eq!(
            headers.iter().collect::<Vec<_>>(),
            vec![("X-Custom-Token", "abc 123\ttab")]
        );
    }

    #[test]
    fn rejects_a_malformed_name() {
        // Empty, and names containing separators/control bytes, are not tokens.
        assert_eq!(
            make("", "v").unwrap_err().reason,
            "not a valid HTTP header name"
        );
        assert_eq!(
            make("bad name", "v").unwrap_err().reason,
            "not a valid HTTP header name"
        );
        assert_eq!(
            make("colon:name", "v").unwrap_err().reason,
            "not a valid HTTP header name"
        );
    }

    #[test]
    fn rejects_a_value_that_would_inject_a_second_header() {
        // A CRLF (or bare newline / NUL) in the value could split the header line;
        // it must be rejected as a control character.
        for injection in ["v\r\nEvil: 1", "v\nEvil: 1", "v\0"] {
            let err = make("X-Test", injection).unwrap_err();
            assert_eq!(err.reason, "value contains control characters");
            assert_eq!(err.name, "X-Test");
        }
    }

    #[test]
    fn into_column_is_none_when_empty_and_some_otherwise() {
        let empty = WebhookHeaders::default();
        assert!(empty.is_empty());
        assert!(empty.into_column().is_none());

        let populated = make("X-A", "1").expect("valid");
        let column = populated.into_column().expect("some for non-empty");
        // The stored blob decodes back to the same headers.
        assert_eq!(column.strict().unwrap(), make("X-A", "1").unwrap());
    }
}
