//! Validated JSON extractor with automatic validation.
//!
//! This module provides [`ValidateJson`], an enhanced JSON extractor that
//! combines deserialization with automatic validation using the `validator` crate.

use std::borrow::Cow;
use std::collections::HashMap;

use axum::extract::{FromRequest, Request};
use derive_more::{Deref, DerefMut, From};
use serde::de::DeserializeOwned;
use validator::{Validate, ValidationErrors};

use super::Json;
use crate::handler::{Error, ErrorKind};

/// Enhanced JSON extractor with automatic validation using the `validator` crate.
///
/// This extractor combines JSON deserialization with automatic validation,
/// providing comprehensive error messages for validation failures. It works
/// with any type that implements both `serde::Deserialize` and `validator::Validate`.
///
/// Also see [`Json`]
///
/// [`Json`]: axum::extract::Json
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct ValidateJson<T>(pub T);

impl<T> ValidateJson<T> {
    /// Creates a new instance of [`ValidateJson`].
    #[inline]
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Returns the inner validated value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, S> FromRequest<S> for ValidateJson<T>
where
    T: DeserializeOwned + Validate + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // First, deserialize the JSON
        let Json(data) = <Json<T> as FromRequest<S>>::from_request(req, state).await?;

        // Then validate the deserialized data
        data.validate()?;
        Ok(Self::new(data))
    }
}

/// Formats length validation errors with appropriate units and context.
fn format_length_error(
    field: &str,
    params: &HashMap<Cow<'static, str>, serde_json::Value>,
) -> String {
    if params.is_empty() {
        return format!("Field '{}' has invalid length", field);
    }

    // Determine if we're dealing with characters or items
    let unit = if field.contains("password") || field.contains("text") || field.contains("name") {
        "characters"
    } else {
        "items"
    };

    match (params.get("min"), params.get("max")) {
        (Some(min), Some(max)) => {
            let min_val = extract_number_from_json(min).unwrap_or(0.0) as u64;
            let max_val = extract_number_from_json(max).unwrap_or(0.0) as u64;
            format!(
                "Field '{}' must be between {} and {} {} long",
                field, min_val, max_val, unit
            )
        }
        (Some(min), None) => {
            let min_val = extract_number_from_json(min).unwrap_or(0.0) as u64;
            format!(
                "Field '{}' must be at least {} {} long",
                field, min_val, unit
            )
        }
        (None, Some(max)) => {
            let max_val = extract_number_from_json(max).unwrap_or(0.0) as u64;
            format!(
                "Field '{}' must be at most {} {} long",
                field, max_val, unit
            )
        }
        _ => format!("Field '{}' has invalid length", field),
    }
}

/// Formats range validation errors with appropriate context and units.
fn format_range_error(
    field: &str,
    params: &HashMap<Cow<'static, str>, serde_json::Value>,
) -> String {
    if params.is_empty() {
        return format!("Field '{}' is out of valid range", field);
    }

    match (params.get("min"), params.get("max")) {
        (Some(min), Some(max)) => {
            let min_val = extract_number_from_json(min).unwrap_or(0.0);
            let max_val = extract_number_from_json(max).unwrap_or(0.0);
            format!(
                "Field '{}' must be between {} and {}",
                field, min_val, max_val
            )
        }
        (Some(min), None) => {
            let min_val = extract_number_from_json(min).unwrap_or(0.0);
            format!("Field '{}' must be at least {}", field, min_val)
        }
        (None, Some(max)) => {
            let max_val = extract_number_from_json(max).unwrap_or(0.0);
            format!("Field '{}' must be at most {}", field, max_val)
        }
        _ => format!("Field '{}' is out of valid range", field),
    }
}

/// Extracts a number from a JSON value, supporting both integers and floats.
fn extract_number_from_json(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// Formats validation errors with context-aware, user-friendly messages.
fn format_validation_error(field: &str, error: &validator::ValidationError) -> String {
    // Use custom message if provided, otherwise generate appropriate message
    if let Some(custom_message) = &error.message {
        return format!("Field '{}': {}", field, custom_message);
    }

    let message = match error.code.as_ref() {
        "required" => "is required and cannot be empty".to_string(),
        "length" => return format_length_error(field, &error.params),
        "email" => "must be a valid email address (e.g., user@example.com)".to_string(),
        "range" => return format_range_error(field, &error.params),
        "url" => "must be a valid URL (e.g., https://example.com)".to_string(),
        "phone" => "must be a valid phone number in international format".to_string(),
        "credit_card" => "must be a valid credit card number".to_string(),
        "must_match" => {
            let other_field = error
                .params
                .get("other")
                .and_then(|v| v.as_str())
                .unwrap_or("other field");
            format!("must match '{}'", other_field)
        }
        "regex" => "format is invalid - please check the required pattern".to_string(),
        "contains" => {
            let needle = error
                .params
                .get("needle")
                .and_then(|v| v.as_str())
                .unwrap_or("required text");
            format!("must contain '{}'", needle)
        }
        "does_not_contain" => {
            let needle = error
                .params
                .get("needle")
                .and_then(|v| v.as_str())
                .unwrap_or("forbidden text");
            format!("must not contain '{}'", needle)
        }
        code => format!("failed validation: {}", code),
    };

    format!("Field '{}' {}", field, message)
}

impl From<ValidationErrors> for Error<'static> {
    fn from(errors: ValidationErrors) -> Self {
        let error_messages: Vec<String> = errors
            .field_errors()
            .iter()
            .flat_map(|(field, field_errors)| {
                field_errors
                    .iter()
                    .map(move |error| format_validation_error(field, error))
            })
            .collect();

        // Show validation details in the user-facing message
        let user_message = match error_messages.as_slice() {
            [] => "Validation failed".to_string(),
            [single_error] => single_error.clone(),
            multiple => multiple.join(". "),
        };

        tracing::warn!(
            errors = ?errors.field_errors(),
            "Request validation failed"
        );

        ErrorKind::BadRequest
            .with_message(user_message)
            .with_resource("request")
    }
}

impl<T> aide::OperationInput for ValidateJson<T>
where
    T: schemars::JsonSchema,
{
    fn operation_input(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) {
        Json::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<u16>, aide::openapi::Response)> {
        Json::<T>::inferred_early_responses(ctx, operation)
    }
}
