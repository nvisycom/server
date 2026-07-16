//! Workspace slug: a URL-safe, human-readable workspace identifier.

use std::str::FromStr;

use derive_more::{AsRef, Display, Into};
use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use serde::{Deserialize, Serialize};

/// Minimum length of a workspace slug, in characters.
pub const SLUG_MIN_LENGTH: usize = 3;

/// Maximum length of a workspace slug, in characters.
pub const SLUG_MAX_LENGTH: usize = 32;

/// Error returned when a string is not a valid [`WorkspaceSlug`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SlugError {
    /// The slug is shorter than [`SLUG_MIN_LENGTH`] or longer than
    /// [`SLUG_MAX_LENGTH`].
    #[error("slug must be between {SLUG_MIN_LENGTH} and {SLUG_MAX_LENGTH} characters")]
    Length,
    /// The slug contains characters other than `[a-z0-9-]`, or has a leading,
    /// trailing, or doubled dash.
    #[error("slug must be lowercase alphanumeric with single internal dashes")]
    Format,
}

/// A validated workspace slug.
///
/// A slug is the human-readable identifier for a workspace in URLs
/// (`/workspaces/{slug}/...`). It is lowercase, dash-separated, and unique
/// across the platform. The invariants — `[a-z0-9]` with single internal
/// dashes, length [`SLUG_MIN_LENGTH`]–[`SLUG_MAX_LENGTH`] — are enforced on
/// construction, so an existing `WorkspaceSlug` is always valid. The matching
/// database `CHECK` mirrors this exact shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, AsRef, Into)]
#[derive(Serialize, Deserialize, AsExpression, FromSqlRow)]
#[as_ref(str)]
#[diesel(sql_type = Text)]
#[serde(try_from = "String", into = "String")]
pub struct WorkspaceSlug(String);

impl WorkspaceSlug {
    /// Validates `value` and wraps it as a [`WorkspaceSlug`].
    ///
    /// # Errors
    ///
    /// Returns [`SlugError`] if `value` is the wrong length or not in canonical
    /// slug form.
    pub fn parse(value: impl Into<String>) -> Result<Self, SlugError> {
        let value = value.into();
        Self::validate(&value)?;
        Ok(Self(value))
    }

    /// Derives a canonical slug from arbitrary text (e.g. a display name).
    ///
    /// The text is slugified, then truncated to [`SLUG_MAX_LENGTH`] on a dash
    /// boundary where possible. Returns `None` if the result cannot satisfy the
    /// minimum length (e.g. the input has no slug-able characters).
    pub fn derive(text: &str) -> Option<Self> {
        let slugged = slug::slugify(text);
        let trimmed = truncate_on_dash(&slugged, SLUG_MAX_LENGTH);
        Self::parse(trimmed).ok()
    }

    /// Returns a variant of this slug disambiguated by a numeric suffix, e.g.
    /// `acme` with `n = 2` becomes `acme-2`.
    ///
    /// The base is truncated if necessary so the suffixed slug still fits within
    /// [`SLUG_MAX_LENGTH`]. Used to resolve slug collisions during generation.
    /// Returns `None` only if `n` is so large the suffix alone cannot fit.
    pub fn with_numeric_suffix(&self, n: u32) -> Option<Self> {
        let suffix = format!("-{n}");
        let budget = SLUG_MAX_LENGTH.checked_sub(suffix.len())?;
        if budget < SLUG_MIN_LENGTH {
            return None;
        }

        let base = truncate_on_dash(&self.0, budget);
        Self::parse(format!("{base}{suffix}")).ok()
    }

    /// Returns the slug as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the slug, returning the inner [`String`].
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Checks the slug invariants without allocating.
    fn validate(value: &str) -> Result<(), SlugError> {
        let length = value.chars().count();
        if !(SLUG_MIN_LENGTH..=SLUG_MAX_LENGTH).contains(&length) {
            return Err(SlugError::Length);
        }

        let valid_chars = value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        let bounded = !value.starts_with('-') && !value.ends_with('-');
        let no_double_dash = !value.contains("--");

        if valid_chars && bounded && no_double_dash {
            Ok(())
        } else {
            Err(SlugError::Format)
        }
    }
}

/// Truncates a slug to at most `max` characters, preferring to cut on a dash so
/// the result never ends mid-word or with a trailing dash.
fn truncate_on_dash(slug: &str, max: usize) -> String {
    if slug.chars().count() <= max {
        return slug.to_owned();
    }

    let head: String = slug.chars().take(max).collect();
    match head.rfind('-') {
        Some(idx) if idx >= SLUG_MIN_LENGTH => head[..idx].to_owned(),
        _ => head.trim_end_matches('-').to_owned(),
    }
}

impl FromStr for WorkspaceSlug {
    type Err = SlugError;

    #[inline]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<String> for WorkspaceSlug {
    type Error = SlugError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<DB> ToSql<Text, DB> for WorkspaceSlug
where
    DB: Backend,
    str: ToSql<Text, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> serialize::Result {
        self.0.as_str().to_sql(out)
    }
}

impl<DB> FromSql<Text, DB> for WorkspaceSlug
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
impl schemars::JsonSchema for WorkspaceSlug {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WorkspaceSlug".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": r"^[a-z0-9]+(?:-[a-z0-9]+)*$",
            "minLength": SLUG_MIN_LENGTH,
            "maxLength": SLUG_MAX_LENGTH,
            "description": "Lowercase, dash-separated workspace identifier used in URLs.",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_slugs() {
        for value in ["acme", "acme-corp", "a1-b2-c3", "team-42"] {
            assert!(WorkspaceSlug::parse(value).is_ok(), "should accept {value}");
        }
    }

    #[test]
    fn rejects_malformed_slugs() {
        let cases = [
            ("ab", SlugError::Length),            // too short
            (&"a".repeat(33), SlugError::Length), // too long
            ("Acme", SlugError::Format),          // uppercase
            ("acme_corp", SlugError::Format),     // underscore
            ("-acme", SlugError::Format),         // leading dash
            ("acme-", SlugError::Format),         // trailing dash
            ("acme--corp", SlugError::Format),    // doubled dash
            ("acme corp", SlugError::Format),     // space
        ];
        for (value, expected) in cases {
            assert_eq!(WorkspaceSlug::parse(value), Err(expected), "value: {value}");
        }
    }

    #[test]
    fn derives_from_display_name() {
        assert_eq!(
            WorkspaceSlug::derive("Acme Corp").unwrap().as_str(),
            "acme-corp"
        );
        assert_eq!(
            WorkspaceSlug::derive("  Hello_World  ").unwrap().as_str(),
            "hello-world"
        );
    }

    #[test]
    fn derive_truncates_on_a_dash_boundary() {
        // 40 chars of words; truncated to <=32 without a trailing dash.
        let slug = WorkspaceSlug::derive("alpha beta gamma delta epsilon zeta").unwrap();
        assert!(slug.as_str().len() <= SLUG_MAX_LENGTH);
        assert!(!slug.as_str().ends_with('-'));
        assert!(!slug.as_str().contains("--"));
    }

    #[test]
    fn derive_returns_none_without_sluggable_characters() {
        assert!(WorkspaceSlug::derive("!!!").is_none());
        assert!(WorkspaceSlug::derive("").is_none());
    }

    #[test]
    fn round_trips_through_string() {
        let slug = WorkspaceSlug::parse("acme-corp").unwrap();
        let string: String = slug.clone().into();
        assert_eq!(WorkspaceSlug::try_from(string).unwrap(), slug);
    }

    #[test]
    fn numeric_suffix_appends_and_stays_valid() {
        let slug = WorkspaceSlug::parse("acme").unwrap();
        assert_eq!(slug.with_numeric_suffix(2).unwrap().as_str(), "acme-2");
        assert_eq!(slug.with_numeric_suffix(17).unwrap().as_str(), "acme-17");
    }

    #[test]
    fn numeric_suffix_truncates_long_base_to_fit() {
        // 32-char base leaves no room for "-2" without truncation.
        let slug = WorkspaceSlug::parse("a".repeat(SLUG_MAX_LENGTH)).unwrap();
        let suffixed = slug.with_numeric_suffix(2).unwrap();
        assert!(suffixed.as_str().len() <= SLUG_MAX_LENGTH);
        assert!(suffixed.as_str().ends_with("-2"));
        assert!(!suffixed.as_str().contains("--"));
    }
}
