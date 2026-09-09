//! Shared `garde` custom validators for request DTOs.
//!
//! These are referenced from field attributes as
//! `#[garde(custom(validate_non_blank))]`. A garde custom validator has the
//! signature `fn(&T, &Context) -> garde::Result`, where `T` is the field type
//! and the context is `()` for our DTOs. The returned [`garde::Error`] carries
//! only a message — never the submitted value — so a failure cannot leak
//! request contents.

/// Rejects a value that is empty once trimmed, matching the database's
/// non-empty-trimmed constraint on display names.
///
/// A `length(min = 1)` rule counts characters, so a whitespace-only value passes
/// it; this rule rejects such a value up front with a clean `400` rather than
/// letting it reach the database's `trim()` constraint (a late failure).
pub fn validate_non_blank(value: &str, _: &()) -> garde::Result {
    if value.trim().is_empty() {
        return Err(garde::Error::new("must not be blank"));
    }
    Ok(())
}

/// The [`validate_non_blank`] check for an optional field.
///
/// Takes an `Option` because garde passes a custom validator the field value
/// as-is (it does not unwrap `Option` the way the built-in rules do). An absent
/// value is nothing to check; a present value must not be blank once trimmed.
pub fn validate_non_blank_opt(value: &Option<String>, ctx: &()) -> garde::Result {
    match value {
        Some(value) => validate_non_blank(value, ctx),
        None => Ok(()),
    }
}

/// Restricts a display name to letters, digits, whitespace, hyphens, and
/// apostrophes.
///
/// Takes an `Option` because garde passes a custom validator the field value
/// as-is — it does not unwrap `Option` the way the built-in rules do — and this
/// validator is applied to optional name fields; an absent name is nothing to
/// check.
pub fn validate_display_name_format(name: &Option<String>, _: &()) -> garde::Result {
    let Some(name) = name else {
        return Ok(());
    };
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '\'')
    {
        return Err(garde::Error::new(
            "may contain only letters, digits, spaces, hyphens, and apostrophes",
        ));
    }
    Ok(())
}
