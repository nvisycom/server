//! Username: a public, human-facing account handle.

use std::str::FromStr;

use derive_more::{AsRef, Display, Into};
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};

/// Minimum length of a username, in characters.
pub const USERNAME_MIN_LENGTH: usize = 3;

/// Maximum length of a username, in characters.
pub const USERNAME_MAX_LENGTH: usize = 32;

/// Error returned when a string is not a valid [`Username`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsernameError {
    /// The username is shorter than [`USERNAME_MIN_LENGTH`] or longer than
    /// [`USERNAME_MAX_LENGTH`].
    #[error("username must be between {USERNAME_MIN_LENGTH} and {USERNAME_MAX_LENGTH} characters")]
    Length,
    /// The username contains characters other than `[a-z0-9-]`, or has a
    /// leading, trailing, or doubled dash.
    #[error("username must be lowercase alphanumeric with single internal dashes")]
    Format,
}

/// A validated, public account handle.
///
/// The username is the human-facing identity of an account: it addresses the
/// public profile at `/u/{username}` and appears in place of the account's
/// database id everywhere the API refers to an account. Unlike a
/// [`Slug`](crate::types::Slug), a username is globally unique and may be
/// changed by its owner. The invariants — `[a-z0-9]` with single internal
/// dashes, length [`USERNAME_MIN_LENGTH`]–[`USERNAME_MAX_LENGTH`] — are enforced
/// on construction, so an existing `Username` is always valid; the matching
/// database `CHECK` mirrors this exact shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, AsRef, Into)]
#[derive(Serialize, Deserialize, AsExpression, FromSqlRow)]
#[as_ref(str)]
#[diesel(sql_type = Text)]
#[serde(try_from = "String", into = "String")]
pub struct Username(String);

impl Username {
    /// Validates `value` and wraps it as a [`Username`].
    ///
    /// # Errors
    ///
    /// Returns [`UsernameError`] if `value` is the wrong length or not in
    /// canonical handle form.
    pub fn parse(value: impl Into<String>) -> Result<Self, UsernameError> {
        let value = value.into();
        Self::validate(&value)?;
        Ok(Self(value))
    }

    /// Returns the username as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the username, returning the inner [`String`].
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Checks the username invariants without allocating.
    fn validate(value: &str) -> Result<(), UsernameError> {
        let length = value.chars().count();
        if !(USERNAME_MIN_LENGTH..=USERNAME_MAX_LENGTH).contains(&length) {
            return Err(UsernameError::Length);
        }

        let valid_chars = value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        let bounded = !value.starts_with('-') && !value.ends_with('-');
        let no_double_dash = !value.contains("--");

        if valid_chars && bounded && no_double_dash {
            Ok(())
        } else {
            Err(UsernameError::Format)
        }
    }
}

impl FromStr for Username {
    type Err = UsernameError;

    #[inline]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<String> for Username {
    type Error = UsernameError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<DB> ToSql<Text, DB> for Username
where
    DB: Backend,
    str: ToSql<Text, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> serialize::Result {
        self.0.as_str().to_sql(out)
    }
}

impl<DB> FromSql<Text, DB> for Username
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
impl schemars::JsonSchema for Username {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Username".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": r"^[a-z0-9]+(?:-[a-z0-9]+)*$",
            "minLength": USERNAME_MIN_LENGTH,
            "maxLength": USERNAME_MAX_LENGTH,
            "description": "Public account handle used in URLs (e.g. /u/{username}).",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_usernames() {
        for value in ["ada", "ada-lovelace", "user-42", "a1b2c3"] {
            assert!(Username::parse(value).is_ok(), "should accept {value}");
        }
    }

    #[test]
    fn rejects_malformed_usernames() {
        let cases = [
            ("ab", UsernameError::Length),            // too short
            (&"a".repeat(33), UsernameError::Length), // too long
            ("Ada", UsernameError::Format),           // uppercase
            ("ada_lovelace", UsernameError::Format),  // underscore
            ("-ada", UsernameError::Format),          // leading dash
            ("ada-", UsernameError::Format),          // trailing dash
            ("ada--lovelace", UsernameError::Format), // doubled dash
            ("ada lovelace", UsernameError::Format),  // space
        ];
        for (value, expected) in cases {
            assert_eq!(Username::parse(value), Err(expected), "value: {value}");
        }
    }

    #[test]
    fn round_trips_through_string() {
        let username = Username::parse("ada-lovelace").unwrap();
        let string: String = username.clone().into();
        assert_eq!(Username::try_from(string).unwrap(), username);
    }
}
