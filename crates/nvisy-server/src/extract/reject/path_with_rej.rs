//! Enhanced path parameter extractor with improved error handling.
//!
//! This module provides [`Path`], an enhanced version of [`axum::extract::Path`]
//! with better error messages and OpenAPI documentation support.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, Response};
use axum::extract::rejection::PathRejection;
use axum::extract::{FromRequestParts, Path as AxumPath};
use derive_more::{Deref, DerefMut, From};
// `FromRequestParts` above is both the trait and the derive macro re-exported by
// axum's `macros` feature; the derive on `Path` resolves to it.
use schemars::JsonSchema;

use super::sanitize_error_message;
use crate::response::{Error, ErrorKind};

/// Enhanced path parameter extractor with improved error handling.
///
/// This extractor provides better error messages compared to the
/// default Axum [`Path`] extractor. It includes:
///
/// - Detailed error messages for different parameter types
/// - Type-safe deserialization with proper error context
///
/// The [`FromRequestParts`] impl is derived: extraction delegates to
/// [`axum::extract::Path`] and its [`PathRejection`] is mapped into our
/// [`Error`] by the `From` impl below (that mapping is where the improved
/// messages live).
///
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging and user experience.
///
/// [`Path`]: AxumPath
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From, FromRequestParts)]
#[from_request(via(AxumPath), rejection(Error<'static>))]
pub struct Path<T>(pub T);

impl From<PathRejection> for Error<'static> {
    fn from(rejection: PathRejection) -> Self {
        match rejection {
            PathRejection::FailedToDeserializePathParams(err) => {
                let error_message = sanitize_error_message(&err.to_string());

                tracing::warn!(
                    error = %error_message,
                    "Path parameter deserialization failed"
                );

                ErrorKind::BadRequest
                    .with_message("Invalid path parameter format")
                    .with_context(format!(
                        "Path parameter deserialization failed: {}. Check that the parameter matches the expected type.",
                        error_message
                    ))
            }
            PathRejection::MissingPathParams(err) => {
                let error_message = sanitize_error_message(&err.to_string());

                tracing::warn!(
                    error = %error_message,
                    "Missing path parameter"
                );

                ErrorKind::MissingPathParam
                    .with_message("Required path parameter missing")
                    .with_context(format!(
                        "Path parameter extraction failed: {}. Ensure all required parameters are present in the URL path and match the expected route pattern.",
                        error_message
                    ))
            }
            _ => {
                tracing::error!("Unexpected path rejection error");

                ErrorKind::InternalServerError
                    .with_message("Path processing failed")
                    .with_context("Unexpected error occurred during path parameter processing. This may indicate a routing configuration issue.")
            }
        }
    }
}

impl<T> OperationInput for Path<T>
where
    T: JsonSchema,
{
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        AxumPath::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        AxumPath::<T>::inferred_early_responses(ctx, operation)
    }
}
