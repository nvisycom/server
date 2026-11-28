//! A wrapper that keeps a secret out of `Debug` output.

#[cfg(feature = "schema")]
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The placeholder every redacted value prints as.
pub const REDACTED: &str = "<redacted>";

/// A secret value (a password, API key, token, or client secret) whose `Debug`
/// output is [`REDACTED`], so a derived `Debug` on the struct holding it, or a
/// `tracing` field, cannot leak it.
///
/// Only `Debug` is guarded: it parses, serializes, deserializes, and describes
/// its schema exactly as the inner value, so a config field changes type without
/// changing its wire or CLI form. Read the value with [`expose`](Self::expose).
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wraps a secret value.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// The secret value.
    #[must_use]
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Unwraps the secret value.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: FromStr> FromStr for Secret<T> {
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

#[cfg(feature = "schema")]
impl<T: schemars::JsonSchema> schemars::JsonSchema for Secret<T> {
    fn inline_schema() -> bool {
        T::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        T::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        T::json_schema(generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_value() {
        let secret = Secret::new("hunter2");
        assert_eq!(format!("{secret:?}"), REDACTED);
        assert_eq!(format!("{:?}", Some(secret)), "Some(<redacted>)");
    }

    #[test]
    fn wire_and_parse_forms_are_the_inner_value() {
        let secret: Secret<String> = "hunter2".parse().unwrap();
        assert_eq!(secret.expose(), "hunter2");
        assert_eq!(serde_json::to_string(&secret).unwrap(), r#""hunter2""#);
        let back: Secret<String> = serde_json::from_str(r#""hunter2""#).unwrap();
        assert_eq!(back, secret);
    }
}
