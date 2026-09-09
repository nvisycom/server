//! Enhanced form data extractor with improved error handling.
//!
//! This module provides [`Form`], an enhanced version of [`axum::Form`] with
//! better error messages and OpenAPI documentation support.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, Response};
use axum::extract::rejection::FormRejection;
use axum::extract::{Form as AxumForm, FromRequest};
use derive_more::{Deref, DerefMut, From};
use schemars::JsonSchema;

use super::sanitize_error_message;
use crate::extract::Query;
use crate::handler::{Error, ErrorKind};

/// Enhanced form data extractor with improved error handling.
///
/// This extractor provides better error messages compared to the
/// default Axum [`Form`] extractor. It includes:
///
/// - Detailed error messages for different form parsing failures
/// - Type-safe deserialization with proper error context
/// - Clear indication of which fields failed validation
/// - Content-Type validation with helpful suggestions
///
/// The [`FromRequest`] impl is derived: extraction delegates to [`axum::Form`]
/// and its [`FormRejection`] is mapped into our [`Error`] by the `From` impl
/// below (that mapping is where the improved messages live).
///
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging and user experience.
///
/// [Form]: AxumForm
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From, FromRequest)]
#[from_request(via(AxumForm), rejection(Error<'static>))]
pub struct Form<T>(pub T);

/// Maps a form rejection into a structured bad-request [`Error`].
///
/// The deserializer message is sanitized before it becomes context so that
/// submitted field values are not echoed back or logged.
impl From<FormRejection> for Error<'static> {
    fn from(rejection: FormRejection) -> Self {
        // Sanitize before logging: a deserialization rejection can echo submitted
        // field values, so the raw rejection must never reach the log line.
        let sanitized = sanitize_error_message(&rejection.to_string());
        tracing::debug!(
            target: "nvisy::extract::form",
            error = %sanitized,
            "Form data parsing failed"
        );

        match rejection {
            FormRejection::FailedToDeserializeForm(_) => ErrorKind::BadRequest
                .with_message("Invalid form data")
                .with_context(sanitized),
            FormRejection::InvalidFormContentType(_) => ErrorKind::BadRequest
                .with_message("Invalid content type for form data")
                .with_context(
                    "Expected 'application/x-www-form-urlencoded'. \
                    Set the correct Content-Type header for form submissions",
                ),
            FormRejection::BytesRejection(_) => ErrorKind::BadRequest
                .with_message("Failed to read form data")
                .with_context("The request body could not be read as form data"),
            _ => ErrorKind::BadRequest
                .with_message("Invalid form submission")
                .with_context("The form data could not be processed"),
        }
    }
}

impl<T> OperationInput for Form<T>
where
    T: JsonSchema,
{
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        Query::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        Query::<T>::inferred_early_responses(ctx, operation)
    }
}
