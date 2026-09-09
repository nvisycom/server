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
pub fn validate_non_blank(value: &str, _: &()) -> garde::Result {
    if value.trim().is_empty() {
        return Err(garde::Error::new("must not be blank"));
    }
    Ok(())
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
