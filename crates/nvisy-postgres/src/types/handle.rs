//! Handle: a validated, URL-safe identifier shared by usernames and slugs.

use std::str::FromStr;

use derive_more::{AsRef, Display, Into};
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};

/// Minimum length of a handle, in characters.
pub const HANDLE_MIN_LENGTH: usize = 3;

/// Maximum length of a handle, in characters.
pub const HANDLE_MAX_LENGTH: usize = 32;

/// Error returned when a string is not a valid [`Handle`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandleError {
    /// The value is shorter than [`HANDLE_MIN_LENGTH`] or longer than
    /// [`HANDLE_MAX_LENGTH`].
    #[error("must be between {HANDLE_MIN_LENGTH} and {HANDLE_MAX_LENGTH} characters")]
    Length,
    /// The value contains characters other than `[a-z0-9-]`, or has a leading,
    /// trailing, or doubled dash.
    #[error("must be lowercase alphanumeric with single internal dashes")]
    Format,
}

/// A validated, URL-safe identifier.
///
/// Handles are the human-readable identifiers used both for account usernames
/// and for resource slugs (workspaces, pipelines, policies, and so on). They are
/// lowercase and dash-separated. The invariants — `[a-z0-9]` with single
/// internal dashes, length [`HANDLE_MIN_LENGTH`]–[`HANDLE_MAX_LENGTH`] — are
/// enforced on construction, so an existing `Handle` is always valid; the
/// matching database `CHECK`s mirror this exact shape. Uniqueness scope (global
/// vs per-parent) is enforced by each table's constraint, not by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, AsRef, Into)]
#[derive(Serialize, Deserialize, AsExpression, FromSqlRow)]
#[as_ref(str)]
#[diesel(sql_type = Text)]
#[serde(try_from = "String", into = "String")]
pub struct Handle(String);

impl Handle {
    /// Validates `value` and wraps it as a [`Handle`].
    ///
    /// # Errors
    ///
    /// Returns [`HandleError`] if `value` is the wrong length or not in canonical
    /// handle form.
    pub fn parse(value: impl Into<String>) -> Result<Self, HandleError> {
        let value = value.into();
        Self::validate(&value)?;
        Ok(Self(value))
    }

    /// Derives a canonical handle from arbitrary text (e.g. a display name).
    ///
    /// The text is slugified, then truncated to [`HANDLE_MAX_LENGTH`] on a dash
    /// boundary where possible. Returns `None` if the result cannot satisfy the
    /// minimum length (e.g. the input has no slug-able characters).
    pub fn derive(text: &str) -> Option<Self> {
        let slugged = slug::slugify(text);
        let trimmed = truncate_on_dash(&slugged, HANDLE_MAX_LENGTH);
        Self::parse(trimmed).ok()
    }

    /// Returns the handle as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the handle, returning the inner [`String`].
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Checks the handle invariants without allocating.
    fn validate(value: &str) -> Result<(), HandleError> {
        let length = value.chars().count();
        if !(HANDLE_MIN_LENGTH..=HANDLE_MAX_LENGTH).contains(&length) {
            return Err(HandleError::Length);
        }

        let valid_chars = value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        let bounded = !value.starts_with('-') && !value.ends_with('-');
        let no_double_dash = !value.contains("--");

        if valid_chars && bounded && no_double_dash {
            Ok(())
        } else {
            Err(HandleError::Format)
        }
    }
}

/// Truncates a handle to at most `max` characters, preferring to cut on a dash so
/// the result never ends mid-word or with a trailing dash.
fn truncate_on_dash(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }

    let head: String = value.chars().take(max).collect();
    match head.rfind('-') {
        Some(idx) if idx >= HANDLE_MIN_LENGTH => head[..idx].to_owned(),
        _ => head.trim_end_matches('-').to_owned(),
    }
}

impl FromStr for Handle {
    type Err = HandleError;

    #[inline]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<String> for Handle {
    type Error = HandleError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<DB> ToSql<Text, DB> for Handle
where
    DB: Backend,
    str: ToSql<Text, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> serialize::Result {
        self.0.as_str().to_sql(out)
    }
}

impl<DB> FromSql<Text, DB> for Handle
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let value = String::from_sql(bytes)?;
        Ok(Self::parse(value)?)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for Handle {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Handle".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": r"^[a-z0-9]+(?:-[a-z0-9]+)*$",
            "minLength": HANDLE_MIN_LENGTH,
            "maxLength": HANDLE_MAX_LENGTH,
            "description": "Lowercase, dash-separated identifier used in URLs and as account handles.",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_handles() {
        for value in ["acme", "acme-corp", "a1-b2-c3", "team-42"] {
            assert!(Handle::parse(value).is_ok(), "should accept {value}");
        }
    }

    #[test]
    fn rejects_malformed_handles() {
        let cases = [
            ("ab", HandleError::Length),            // too short
            (&"a".repeat(33), HandleError::Length), // too long
            ("Acme", HandleError::Format),          // uppercase
            ("acme_corp", HandleError::Format),     // underscore
            ("-acme", HandleError::Format),         // leading dash
            ("acme-", HandleError::Format),         // trailing dash
            ("acme--corp", HandleError::Format),    // doubled dash
            ("acme corp", HandleError::Format),     // space
        ];
        for (value, expected) in cases {
            assert_eq!(Handle::parse(value), Err(expected), "value: {value}");
        }
    }

    #[test]
    fn derives_from_display_name() {
        assert_eq!(Handle::derive("Acme Corp").unwrap().as_str(), "acme-corp");
        assert_eq!(
            Handle::derive("  Hello_World  ").unwrap().as_str(),
            "hello-world"
        );
    }

    #[test]
    fn derive_truncates_on_a_dash_boundary() {
        // 40 chars of words; truncated to <=32 without a trailing dash.
        let handle = Handle::derive("alpha beta gamma delta epsilon zeta").unwrap();
        assert!(handle.as_str().len() <= HANDLE_MAX_LENGTH);
        assert!(!handle.as_str().ends_with('-'));
        assert!(!handle.as_str().contains("--"));
    }

    #[test]
    fn derive_returns_none_without_sluggable_characters() {
        assert!(Handle::derive("!!!").is_none());
        assert!(Handle::derive("").is_none());
    }

    #[test]
    fn round_trips_through_string() {
        let handle = Handle::parse("acme-corp").unwrap();
        let string: String = handle.clone().into();
        assert_eq!(Handle::try_from(string).unwrap(), handle);
    }
}
