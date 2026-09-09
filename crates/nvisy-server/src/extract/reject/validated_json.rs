//! Validated JSON extractor with automatic validation.
//!
//! This module provides [`ValidateJson`], a JSON extractor that deserializes a
//! body (via [`Json`]) and then runs `validator::Validate` on it, turning any
//! [`ValidationErrors`] into a structured [`Error`].
//!
//! Two invariants shape the error mapping:
//!
//! - **No submitted values escape.** `validator` records the rejected field
//!   value in `params["value"]` for several validators (`length`, `custom`,
//!   `credit_card`, …). Neither the user-facing message nor the log line ever
//!   reads `params["value"]`, so request contents cannot leak through a
//!   validation failure.
//! - **Nested errors are preserved.** Validation of `#[validate(nested)]` fields
//!   produces a tree, not a flat map, so the mapping walks the tree and reports
//!   each leaf under its dotted path (`address.zip`, `items[2].name`).

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write as _;

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, Response};
use axum::extract::{FromRequest, OptionalFromRequest, Request};
use derive_more::{Deref, DerefMut, From};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;
use validator::{Validate, ValidationError, ValidationErrors, ValidationErrorsKind};

use super::Json;
use crate::handler::{Error, ErrorKind};

/// JSON extractor that deserializes and then validates the request body.
///
/// Works with any type that implements both [`serde::Deserialize`] and
/// [`validator::Validate`]. Deserialization is delegated to [`Json`], so JSON
/// syntax and content-type errors carry that extractor's messages; a body that
/// parses but fails validation is rejected with a field-by-field message built
/// by [`ValidationErrors`] mapping below.
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct ValidateJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidateJson<T>
where
    T: DeserializeOwned + Validate + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = <Json<T> as FromRequest<S>>::from_request(req, state).await?;
        data.validate()?;
        Ok(Self(data))
    }
}

impl<T, S> OptionalFromRequest<S> for ValidateJson<T>
where
    T: DeserializeOwned + Validate + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    /// Extracts and validates a body when one is present; an absent or malformed
    /// body yields `None`. Mirrors [`Json`]'s optional semantics: only a server
    /// error propagates, so a missing optional body is not an error.
    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        match <Self as FromRequest<S>>::from_request(req, state).await {
            Ok(validated) => Ok(Some(validated)),
            // For optional extraction, only propagate server errors; client errors
            // (absent body, malformed JSON, validation failure) result in `None`.
            Err(error) if error.kind() == ErrorKind::InternalServerError => Err(error),
            Err(_) => Ok(None),
        }
    }
}

impl From<ValidationErrors> for Error<'static> {
    fn from(errors: ValidationErrors) -> Self {
        let mut leaves = Vec::new();
        collect_leaf_errors(&mut String::new(), &errors, &mut leaves);

        // Log field paths and codes only — never `params`, which can hold the
        // submitted value for `length`/`custom`/`credit_card`/… validators.
        let logged: Vec<String> = leaves
            .iter()
            .map(|(path, error)| format!("{}:{}", path, error.code))
            .collect();
        tracing::warn!(errors = ?logged, "Request validation failed");

        let messages: Vec<String> = leaves
            .iter()
            .map(|(path, error)| describe_error(path, error))
            .collect();

        let user_message = match messages.as_slice() {
            [] => "Validation failed".to_string(),
            [single] => single.clone(),
            many => many.join(". "),
        };

        ErrorKind::BadRequest
            .with_message(user_message)
            .with_resource("request")
    }
}

/// Walks the validation-error tree, pushing each leaf as `(dotted path, error)`.
///
/// `prefix` is the path accumulated from enclosing structs and list indices;
/// leaves at the top level are reported under their bare field name.
fn collect_leaf_errors<'a>(
    prefix: &mut String,
    errors: &'a ValidationErrors,
    leaves: &mut Vec<(String, &'a ValidationError)>,
) {
    for (field, kind) in errors.errors() {
        match kind {
            ValidationErrorsKind::Field(field_errors) => {
                let path = join_path(prefix, field);
                for error in field_errors {
                    leaves.push((path.clone(), error));
                }
            }
            ValidationErrorsKind::Struct(nested) => {
                let mut nested_prefix = join_path(prefix, field);
                collect_leaf_errors(&mut nested_prefix, nested, leaves);
            }
            ValidationErrorsKind::List(items) => {
                for (index, nested) in items {
                    let mut nested_prefix = join_path(prefix, field);
                    let _ = write!(nested_prefix, "[{}]", index);
                    collect_leaf_errors(&mut nested_prefix, nested, leaves);
                }
            }
        }
    }
}

/// Joins a path prefix and a field segment with a `.`, or returns the bare
/// segment when there is no prefix.
fn join_path(prefix: &str, field: &str) -> String {
    if prefix.is_empty() {
        field.to_string()
    } else {
        format!("{}.{}", prefix, field)
    }
}

/// Renders one validation error into a user-facing message.
///
/// A custom message on the error wins. Otherwise the code is mapped for the
/// validators actually used on request DTOs (`length`, `range`, `email`,
/// `url`); anything else gets a neutral fallback. No branch reads
/// `params["value"]`, so a submitted value never reaches the response.
fn describe_error(field: &str, error: &ValidationError) -> String {
    if let Some(message) = &error.message {
        return format!("Field '{}': {}", field, message);
    }

    match error.code.as_ref() {
        "length" => format!("Field '{}' {}", field, format_bounds(&error.params, "long")),
        "range" => format!("Field '{}' {}", field, format_bounds(&error.params, "")),
        "email" => format!(
            "Field '{}' must be a valid email address (e.g., user@example.com)",
            field
        ),
        "url" => format!(
            "Field '{}' must be a valid URL (e.g., https://example.com)",
            field
        ),
        _ => format!("Field '{}' is invalid", field),
    }
}

/// Renders the `min`/`max` bounds shared by `length` and `range` errors.
///
/// `suffix` is appended after each bound (e.g. `"long"` for lengths, empty for
/// ranges). Bounds that are absent or non-numeric fall back to a generic phrase.
fn format_bounds(params: &HashMap<Cow<'static, str>, Value>, suffix: &str) -> String {
    let tail = if suffix.is_empty() {
        String::new()
    } else {
        format!(" {}", suffix)
    };

    match (number(params, "min"), number(params, "max")) {
        (Some(min), Some(max)) => format!("must be between {} and {}{}", min, max, tail),
        (Some(min), None) => format!("must be at least {}{}", min, tail),
        (None, Some(max)) => format!("must be at most {}{}", max, tail),
        (None, None) => "is out of the allowed range".to_string(),
    }
}

/// Reads a numeric bound parameter, rendering it without a trailing `.0` when it
/// is integral so `length` bounds read as `64` rather than `64.0`.
fn number(params: &HashMap<Cow<'static, str>, Value>, key: &str) -> Option<String> {
    let value = params.get(key)?.as_f64()?;
    if value.fract() == 0.0 {
        Some((value as i64).to_string())
    } else {
        Some(value.to_string())
    }
}

impl<T> OperationInput for ValidateJson<T>
where
    T: JsonSchema,
{
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        Json::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        Json::<T>::inferred_early_responses(ctx, operation)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use serde_json::json;
    use validator::{ValidationError, ValidationErrors, ValidationErrorsKind};

    use super::{collect_leaf_errors, describe_error, format_bounds, number};

    fn params(
        pairs: &[(&'static str, serde_json::Value)],
    ) -> std::collections::HashMap<Cow<'static, str>, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (Cow::Borrowed(*k), v.clone()))
            .collect()
    }

    fn error_with_params(code: &'static str, pairs: &[(&'static str, serde_json::Value)]) -> ValidationError {
        let mut error = ValidationError::new(code);
        for (key, value) in pairs {
            error.add_param(Cow::Borrowed(*key), value);
        }
        error
    }

    #[test]
    fn length_bounds_render_as_integers_with_the_long_suffix() {
        let hint = describe_error(
            "display_name",
            &error_with_params("length", &[("min", json!(2)), ("max", json!(64))]),
        );
        assert_eq!(
            hint,
            "Field 'display_name' must be between 2 and 64 long"
        );
    }

    #[test]
    fn length_with_only_one_bound() {
        assert_eq!(
            describe_error("tags", &error_with_params("length", &[("min", json!(1))])),
            "Field 'tags' must be at least 1 long"
        );
        assert_eq!(
            describe_error("tags", &error_with_params("length", &[("max", json!(5))])),
            "Field 'tags' must be at most 5 long"
        );
    }

    #[test]
    fn range_renders_without_a_suffix() {
        assert_eq!(
            describe_error(
                "age",
                &error_with_params("range", &[("min", json!(0)), ("max", json!(120))])
            ),
            "Field 'age' must be between 0 and 120"
        );
    }

    #[test]
    fn email_and_url_get_canned_messages() {
        assert!(describe_error("email", &ValidationError::new("email"))
            .contains("valid email address"));
        assert!(describe_error("homepage", &ValidationError::new("url")).contains("valid URL"));
    }

    #[test]
    fn a_custom_message_wins_over_the_code() {
        let mut custom = ValidationError::new("length");
        custom.message = Some(Cow::Borrowed("Please keep it short"));
        assert_eq!(
            describe_error("bio", &custom),
            "Field 'bio': Please keep it short"
        );
    }

    #[test]
    fn an_unknown_code_gets_a_neutral_fallback() {
        assert_eq!(
            describe_error("x", &ValidationError::new("bespoke_rule")),
            "Field 'x' is invalid"
        );
    }

    #[test]
    fn bounds_fall_back_when_absent_or_non_numeric() {
        assert_eq!(format_bounds(&params(&[]), "long"), "is out of the allowed range");
        assert_eq!(
            format_bounds(&params(&[("min", json!("oops"))]), "long"),
            "is out of the allowed range"
        );
    }

    #[test]
    fn number_drops_the_trailing_zero_on_integral_bounds() {
        assert_eq!(number(&params(&[("min", json!(64.0))]), "min").as_deref(), Some("64"));
        assert_eq!(number(&params(&[("min", json!(1.5))]), "min").as_deref(), Some("1.5"));
        assert_eq!(number(&params(&[]), "min"), None);
    }

    #[test]
    fn nested_struct_errors_are_reported_under_a_dotted_path() {
        // Build: { address: Struct { zip: Field[length] } }
        let mut inner = ValidationErrors::new();
        inner.add("zip", error_with_params("length", &[("min", json!(5))]));

        let mut outer = ValidationErrors::new();
        outer
            .errors_mut()
            .insert(Cow::Borrowed("address"), ValidationErrorsKind::Struct(Box::new(inner)));

        let mut leaves = Vec::new();
        collect_leaf_errors(&mut String::new(), &outer, &mut leaves);

        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].0, "address.zip");
        assert_eq!(leaves[0].1.code.as_ref(), "length");
    }

    #[test]
    fn list_errors_are_reported_with_an_index() {
        // Build: { items: List { 2 => Struct { name: Field[length] } } }
        let mut item = ValidationErrors::new();
        item.add("name", error_with_params("length", &[("max", json!(10))]));

        let mut list = std::collections::BTreeMap::new();
        list.insert(2usize, Box::new(item));

        let mut outer = ValidationErrors::new();
        outer
            .errors_mut()
            .insert(Cow::Borrowed("items"), ValidationErrorsKind::List(list));

        let mut leaves = Vec::new();
        collect_leaf_errors(&mut String::new(), &outer, &mut leaves);

        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].0, "items[2].name");
    }
}
