//! Enhanced form data extractor with improved error handling.
//!
//! This module provides [`Form`], an enhanced version of [`axum::Form`] with
//! better error messages and OpenAPI documentation support.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, Response};
use axum::extract::rejection::FormRejection;
use axum::extract::{Form as AxumForm, FromRequest, OptionalFromRequest, Request};
use derive_more::{Deref, DerefMut, From};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

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
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging and user experience.
///
/// [Form]: AxumForm
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct Form<T>(pub T);

impl<T> Form<T> {
    /// Creates a new instance of [`Form`].
    ///
    /// # Arguments
    ///
    /// * `inner` - The deserialized form data
    #[inline]
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Returns the inner form data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, S> FromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match AxumForm::<T>::from_request(req, state).await {
            Ok(AxumForm(form)) => Ok(Form(form)),
            Err(rejection) => Err(enhance_form_error(rejection)),
        }
    }
}

impl<T, S> OptionalFromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        match AxumForm::<T>::from_request(req, state).await {
            Ok(AxumForm(form)) => Ok(Some(Form(form))),
            Err(_) => Ok(None),
        }
    }
}

/// Converts a form rejection into a structured bad-request [`Error`].
///
/// The deserializer message is sanitized before it becomes context so that
/// submitted field values are not echoed back or logged.
fn enhance_form_error(rejection: FormRejection) -> Error<'static> {
    tracing::debug!(
        target: "nvisy::extract::form",
        error = %rejection,
        "Form data parsing failed"
    );

    match rejection {
        FormRejection::FailedToDeserializeForm(err) => ErrorKind::BadRequest
            .with_message("Invalid form data")
            .with_context(sanitize_error_message(&err.to_string())),
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
