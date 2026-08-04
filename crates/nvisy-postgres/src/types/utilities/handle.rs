//! Shared validation for handle-like identifiers (usernames, slugs).
//!
//! Usernames and slugs share the same lexical rules — lowercase ASCII
//! alphanumerics and single internal dashes, within a length range. This module
//! holds the one implementation both use, so the rule lives in a single place.

/// Why a handle failed validation. Callers map this onto their own error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleViolation {
    /// The value's character length is outside the allowed range.
    Length,
    /// The value contains disallowed characters, a leading/trailing dash, or a
    /// double dash.
    Format,
}

/// Checks handle invariants without allocating: the character count must be
/// within `min..=max`, and the value must be lowercase ASCII alphanumeric with
/// only single internal dashes.
pub fn validate_handle(value: &str, min: usize, max: usize) -> Result<(), HandleViolation> {
    let length = value.chars().count();
    if !(min..=max).contains(&length) {
        return Err(HandleViolation::Length);
    }

    let valid_chars = value
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    let bounded = !value.starts_with('-') && !value.ends_with('-');
    let no_double_dash = !value.contains("--");

    if valid_chars && bounded && no_double_dash {
        Ok(())
    } else {
        Err(HandleViolation::Format)
    }
}
